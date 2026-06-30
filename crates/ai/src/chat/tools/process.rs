pub fn kill_process(pid: u32) {
    if cfg!(target_os = "windows") {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        if let Err(e) = cmd.output() {
            eprintln!("[chat] Failed to kill process {} via taskkill: {}", pid, e);
        }

        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let check = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid), "/NH"])
                .output();
            if let Ok(out) = check
                && !String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
            {
                break;
            }
        }
    } else {
        let result = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        if let Err(e) = result {
            eprintln!("[chat] Failed to kill process {} via kill -9: {}", pid, e);
        }
    }
}
