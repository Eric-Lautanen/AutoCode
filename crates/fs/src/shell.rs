// shell.rs -- Autonomous shell command execution.
// Runs commands in background threads and returns output via channels.
// No permission prompting -- fully autonomous per design spec.

use autocode_core::debug_log;

use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
};

use autocode_core::fsutil;
use autocode_core::helpers;
use autocode_core::state::{ShellStatus, ShellTask};

#[derive(Debug)]
pub enum ShellEvent {
    /// A line of stdout/stderr output.
    Output(String),
    /// Command finished.
    Done { exit_code: i32 },
    /// Error spawning the command.
    SpawnError(String),
}

pub fn run_command_in_dir(command: &str, cwd: Option<&str>) -> (ShellTask, Receiver<ShellEvent>) {
    let (tx, rx) = mpsc::channel();
    let (pid_tx, pid_rx) = mpsc::channel();
    let task_id = helpers::generate_id();
    let created_at = helpers::unix_now();
    let cmd_str = command.to_string();
    let cmd_for_task = cmd_str.clone();
    let cwd = cwd.map(|s| s.to_string());

    std::thread::spawn(move || {
        debug_log!("shell: thread start");
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_command_inner(&cmd_str, cwd.as_deref(), &tx, &pid_tx);
        }));
        debug_log!("shell: thread exit");
    });

    // Receive the child PID once the os process is spawned.
    let pid = pid_rx.recv_timeout(std::time::Duration::from_secs(5)).ok();

    let task = ShellTask {
        id: task_id,
        command: cmd_for_task,
        output: String::new(),
        status: ShellStatus::Running,
        created_at,
        pid,
    };

    (task, rx)
}

fn run_command_inner(
    command: &str,
    cwd: Option<&str>,
    tx: &std::sync::mpsc::Sender<ShellEvent>,
    pid_tx: &std::sync::mpsc::Sender<u32>,
) {
    let mut bat_path_to_clean = None;
    let result = if cfg!(target_os = "windows") {
        let bat_path = std::env::temp_dir().join(format!(
            "ac_shell_{}.cmd",
            autocode_core::helpers::generate_id()
        ));
        let script = if command.contains('\n') {
            let mut s = String::with_capacity(command.len() + 32);
            for line in command.lines() {
                let trimmed = line.trim_end();
                if !trimmed.is_empty() {
                    s.push_str(trimmed);
                    s.push_str("\r\n");
                }
            }
            s
        } else {
            command.to_string()
        };
        if let Err(e) = autocode_core::fsutil::write_cmd_script(&bat_path, &script) {
            let _ = tx.send(ShellEvent::SpawnError(format!(
                "Failed to write command script: {}",
                e
            )));
            return;
        }
        autocode_core::fsutil::track_temp_file(bat_path.clone());
        bat_path_to_clean = Some(bat_path.clone());
        let bat_str = bat_path.to_string_lossy().to_string();
        let mut cmd = Command::new("cmd");
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
        }
        cmd.args(["/C", &bat_str]);
        if let Some(ref d) = cwd {
            cmd.current_dir(d);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        if let Some(ref d) = cwd {
            cmd.current_dir(d);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
    };

    match result {
        Err(e) => {
            let _ = tx.send(ShellEvent::SpawnError(e.to_string()));
        }
        Ok(mut child) => {
            let _ = pid_tx.send(child.id());
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let stderr_done = if let Some(err_pipe) = stderr {
                let tx2 = tx.clone();
                let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
                if std::thread::Builder::new()
                    .name("shell-stderr".into())
                    .spawn(move || {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let reader = BufReader::new(err_pipe);
                            for line in reader.lines().map_while(Result::ok) {
                                if tx2
                                    .send(ShellEvent::Output(format!("[stderr] {}", line)))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }));
                        let _ = done_tx.send(());
                    })
                    .is_ok()
                {
                    Some(done_rx)
                } else {
                    None
                }
            } else {
                None
            };
            let mut aborted = false;
            if let Some(out_pipe) = stdout {
                let reader = BufReader::new(out_pipe);
                for line in reader.lines().map_while(Result::ok) {
                    if tx.send(ShellEvent::Output(line)).is_err() {
                        aborted = true;
                        break;
                    }
                }
            }
            if aborted {
                let _ = child.kill();
            }
            let exit_code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            // Wait for stderr reader to finish before sending Done so that all
            // stderr Output events arrive before Done and are not lost.
            if let Some(done_rx) = stderr_done {
                let _ = done_rx.recv_timeout(std::time::Duration::from_secs(5));
            }
            let _ = tx.send(ShellEvent::Done { exit_code });
        }
    }

    // Clean up temp file after the process finishes
    if let Some(p) = bat_path_to_clean {
        let _ = fsutil::remove_file(&p);
        autocode_core::fsutil::untrack_temp_file(&p);
    }
}

/// Known language tags that should NOT be treated as filenames.
const KNOWN_LANG_TAGS: &[&str] = &[
    "rust",
    "rs",
    "toml",
    "json",
    "yaml",
    "yml",
    "xml",
    "html",
    "css",
    "js",
    "ts",
    "tsx",
    "jsx",
    "py",
    "python",
    "sh",
    "bash",
    "zsh",
    "shell",
    "sql",
    "go",
    "java",
    "c",
    "cpp",
    "h",
    "hpp",
    "cs",
    "rb",
    "php",
    "swift",
    "kt",
    "scala",
    "lua",
    "r",
    "perl",
    "dart",
    "dockerfile",
    "makefile",
    "diff",
    "plaintext",
    "text",
    "markdown",
    "md",
    "ini",
    "cfg",
    "conf",
    "env",
    "nix",
    "haskell",
    "hs",
    "elixir",
    "ex",
    "erlang",
    "clj",
    "clojure",
    "vim",
    "fish",
    "powershell",
    "ps1",
    "bat",
    "cmd",
    "psm1",
];

/// Parse AI output for files to create. Returns (filename, content) pairs.
pub fn extract_files(text: &str) -> Vec<(String, String)> {
    // Looks for: ```filename.ext ... ```
    // or markers like:
    // File: path/to/file.rs
    let mut files = Vec::new();
    let mut in_block = false;
    let mut filename = String::new();
    let mut content = String::new();

    for line in text.lines() {
        if !in_block {
            let trimmed = line.trim();
            if trimmed.starts_with("```") && trimmed.len() > 3 {
                let lang_or_file = trimmed.trim_start_matches('`').trim();
                // Skip known language tags (e.g. ```rust, ```python) so they
                // are not misidentified as filenames.
                if KNOWN_LANG_TAGS.contains(&lang_or_file) {
                    continue;
                }
                // If it contains a '.' it's likely a filename.
                if lang_or_file.contains('.') && !lang_or_file.contains(' ') {
                    in_block = true;
                    filename = lang_or_file.to_string();
                    content = String::new();
                }
            }
        } else if line.trim() == "```" {
            if !filename.is_empty() && !content.trim().is_empty() {
                files.push((filename.clone(), content.clone()));
            }
            in_block = false;
            filename = String::new();
            content = String::new();
        } else {
            content.push_str(line);
            content.push('\n');
        }
    }

    files
}

/// Write files extracted from AI output into the project root.
pub fn write_extracted_files(
    root: &str,
    files: &[(String, String)],
    allow_escape: bool,
) -> Vec<String> {
    let root_path = std::path::Path::new(root);
    let mut written = Vec::new();
    for (name, content) in files {
        let target = root_path.join(name);
        let resolved = autocode_core::helpers::resolve_path_write(name, root, allow_escape);
        if autocode_core::helpers::is_blocked_path(&resolved) {
            written.push(format!("{} (BLOCKED: path traversal)", name));
            continue;
        }
        if let Some(parent) = target.parent() {
            let _ = fsutil::create_dir_all(parent);
        }
        match fsutil::write(&target, content) {
            Ok(_) => written.push(name.clone()),
            Err(e) => written.push(format!("{} (ERROR: {})", name, e)),
        }
    }
    written
}
