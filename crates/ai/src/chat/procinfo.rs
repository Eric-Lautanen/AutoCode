// procinfo.rs -- Process liveness and start-time fingerprints.
//
// A background process outlives the app, so after a restart the manager has no
// `Child` handle to wait on -- only a PID. Liveness is therefore probed out of
// band. PIDs get recycled, so a bare "is this PID alive?" check can match an
// unrelated process; the start-time fingerprint captured at spawn lets us tell
// our own process apart from a recycled PID.
//
// No external crates are used: Windows goes through kernel32 FFI (matching
// `autocode_core::utils::sysinfo`), Linux reads /proc, and other Unix systems
// fall back to `kill(pid, 0)`.

/// True if a process with this PID is currently alive.
pub fn process_alive(pid: u32) -> bool {
    imp::alive(pid)
}

/// An opaque fingerprint of when the process started, or `None` when the
/// platform (or permissions) do not allow reading it. Compared at restore time
/// to reject a PID that has since been recycled by an unrelated process.
pub fn process_start_marker(pid: u32) -> Option<String> {
    imp::start_marker(pid)
}

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut c_void;
        fn GetExitCodeProcess(hProcess: *mut c_void, lpExitCode: *mut u32) -> i32;
        fn GetProcessTimes(
            hProcess: *mut c_void,
            lpCreationTime: *mut FileTime,
            lpExitTime: *mut FileTime,
            lpKernelTime: *mut FileTime,
            lpUserTime: *mut FileTime,
        ) -> i32;
        fn CloseHandle(hObject: *mut c_void) -> i32;
    }

    fn open(pid: u32) -> Option<*mut c_void> {
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if h.is_null() { None } else { Some(h) }
    }

    pub fn alive(pid: u32) -> bool {
        let Some(handle) = open(pid) else {
            return false;
        };
        let mut code: u32 = 0;
        let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
        unsafe {
            CloseHandle(handle);
        }
        ok != 0 && code == STILL_ACTIVE
    }

    pub fn start_marker(pid: u32) -> Option<String> {
        let handle = open(pid)?;
        let mut creation = FileTime { low: 0, high: 0 };
        let mut exit = FileTime { low: 0, high: 0 };
        let mut kernel = FileTime { low: 0, high: 0 };
        let mut user = FileTime { low: 0, high: 0 };
        let ok =
            unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
        unsafe {
            CloseHandle(handle);
        }
        if ok == 0 {
            return None;
        }
        let ticks = ((creation.high as u64) << 32) | creation.low as u64;
        Some(format!("win:{}", ticks))
    }
}

#[cfg(target_os = "linux")]
mod imp {
    pub fn alive(pid: u32) -> bool {
        std::path::Path::new(&format!("/proc/{}", pid)).exists()
    }

    pub fn start_marker(pid: u32) -> Option<String> {
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
        // Field 2 (`comm`) may contain spaces and parentheses, so parse the
        // fields after the final ')'. `starttime` is field 22 -> index 19 here.
        let rest = stat.rsplit_once(')')?.1;
        let starttime = rest.split_whitespace().nth(19)?;
        Some(format!("proc:{}", starttime))
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
mod imp {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    pub fn alive(pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        unsafe { kill(pid as i32, 0) == 0 }
    }

    pub fn start_marker(_pid: u32) -> Option<String> {
        None
    }
}

#[cfg(not(any(unix, windows)))]
mod imp {
    pub fn alive(_pid: u32) -> bool {
        false
    }

    pub fn start_marker(_pid: u32) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_alive() {
        assert!(process_alive(std::process::id()));
    }

    #[test]
    fn marker_is_stable_for_this_process() {
        let pid = std::process::id();
        let a = process_start_marker(pid);
        let b = process_start_marker(pid);
        assert_eq!(a, b, "the start marker must not change between reads");
        // On the platforms we support this should actually be readable.
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        assert!(a.is_some());
    }

    #[test]
    fn absurd_pid_is_not_alive() {
        // PID 0 is the idle process / kernel; never a real user process here.
        assert!(!process_alive(0));
    }
}
