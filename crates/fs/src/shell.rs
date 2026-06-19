// shell.rs -- Autonomous shell command execution.
// Runs commands in background threads and returns output via channels.
// No permission prompting -- fully autonomous per design spec.

use std::{
    io::{BufRead, BufReader, Read},
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

/// Characters that are rejected by the shell sanitizer when not explicitly needed.
const SHELL_METACHARACTERS: &[char] = &[';', '&', '|', '>', '<'];

/// Sanitize a shell command by checking for dangerous metacharacters.
/// Returns `Ok(())` if the command is safe, or `Err` with a description of the problem.
///
/// This is a defense-in-depth measure. The AI should not be generating commands
/// with these characters, but if it does, we reject them to prevent injection.
pub fn sanitize_shell(command: &str) -> Result<(), String> {
    for ch in SHELL_METACHARACTERS {
        if command.contains(*ch) {
            return Err(format!(
                "Shell command contains disallowed character '{}'. \
                 Metacharacters like ; && || > < | are not permitted. \
                 Use separate tool calls instead of chaining commands.",
                ch
            ));
        }
    }
    Ok(())
}

pub fn run_command_in_dir(
    command: &str,
    cwd: Option<&str>,
) -> Result<(ShellTask, Receiver<ShellEvent>), String> {
    sanitize_shell(command)?;

    let (tx, rx) = mpsc::channel();
    let (pid_tx, pid_rx) = mpsc::channel();
    let task_id = helpers::generate_id();
    let created_at = helpers::unix_now();
    let cmd_str = command.to_string();
    let cmd_for_task = cmd_str.clone();
    let cwd = cwd.map(|s| s.to_string());

    std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_command_inner(&cmd_str, cwd.as_deref(), &tx, &pid_tx);
        }));
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

    Ok((task, rx))
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
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        if let Some(ref d) = cwd {
            cmd.current_dir(d);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
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
                let mut reader = out_pipe;
                let mut partial = String::new();
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            for &byte in &buf[..n] {
                                if byte == b'\n' {
                                    if !partial.is_empty() {
                                        if tx
                                            .send(ShellEvent::Output(std::mem::take(&mut partial)))
                                            .is_err()
                                        {
                                            aborted = true;
                                            break;
                                        }
                                    }
                                } else if byte == b'\r' {
                                    // Carriage return — progress spinner update.
                                    // Flush whatever we have so far.
                                    if !partial.is_empty() {
                                        if tx
                                            .send(ShellEvent::Output(std::mem::take(&mut partial)))
                                            .is_err()
                                        {
                                            aborted = true;
                                            break;
                                        }
                                    }
                                } else {
                                    partial.push(byte as char);
                                }
                            }
                            if aborted {
                                break;
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                // Flush any remaining partial output.
                if !partial.is_empty() {
                    let _ = tx.send(ShellEvent::Output(partial));
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
