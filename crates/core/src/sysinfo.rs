// sysinfo.rs -- Detect OS, CPU, GPU, RAM, shell, tool availability.
// Windows: raw Win32 FFI (kernel32 + advapi32) + hidden subprocesses.
// Other platforms: std + subprocess fallbacks.
// Results are stored in AppState for persistence across restarts.

use std::sync::OnceLock;

// ── Public persistent data ────────────────────────────────────────────

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct SysInfo {
    pub report: String,
    pub tool_probes: Vec<ToolProbeEntry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolProbeEntry {
    pub name: String,
    pub available: bool,
}

static LIVE_CACHE: OnceLock<SysInfo> = OnceLock::new();

pub fn is_ready() -> bool {
    LIVE_CACHE.get().is_some()
}

pub fn grep_note_from(_info: &SysInfo) -> &'static str {
    ""
}

pub fn shell_tools_note_from(info: &SysInfo) -> String {
    let platform = if cfg!(target_os = "windows") {
        "Windows — use cmd/PowerShell syntax, NOT Unix commands (no head, tail, less etc.)"
    } else {
        "Unix"
    };
    let available: Vec<&str> = info
        .tool_probes
        .iter()
        .filter(|p| p.available && p.name != "rg" && p.name != "grep" && p.name != "findstr")
        .map(|p| p.name.as_str())
        .collect();
    let tool_part = if available.is_empty() {
        "No common dev tools detected on PATH.".to_string()
    } else {
        format!("Available CLI tools: {}", available.join(", "))
    };
    format!("Platform: {}. {}", platform, tool_part)
}

/// Seed the live cache from persisted state (called at startup).
/// Returns true if the persisted data was usable (non-empty report).
pub fn seed_from_persisted(persisted: &SysInfo) -> bool {
    if !persisted.report.is_empty() && !persisted.tool_probes.is_empty() {
        // Force re-detection if the persisted data contains old Unicode
        // symbols that were replaced with ASCII equivalents.
        if persisted.report.contains('\u{2713}') || persisted.report.contains('\u{2717}') {
            return false;
        }
        let _ = LIVE_CACHE.set(persisted.clone());
        true
    } else {
        false
    }
}

/// Start a background detection thread. When done, the result is placed
/// in the live cache and also returned via the channel.
pub fn start_detect() -> std::sync::mpsc::Receiver<SysInfo> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let info = build_sysinfo();
        let _ = LIVE_CACHE.set(info.clone());
        let _ = tx.send(info);
    });
    rx
}

fn build_sysinfo() -> SysInfo {
    let report = build_report();
    let tool_probes = build_tool_probes();
    SysInfo {
        report,
        tool_probes,
    }
}

fn build_report() -> String {
    let mut lines = Vec::new();

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    if let Ok(ver) = std::env::var("OS") {
        lines.push(format!("OS: {} ({}, {})", ver, arch, os));
    } else {
        let ver = os_version();
        lines.push(format!("OS: {} ({}, {})", ver, arch, os));
    }

    lines.push(cpu_info());
    lines.push(memory_info());
    lines.push(gpu_info());
    lines.push(shell_info());

    // Tool summary is appended from tool_probes later in detect flow.
    // For the report string we include it inline for the system prompt.
    let tool_summary = {
        let probes = build_tool_probes();
        let parts: Vec<String> = probes
            .iter()
            .map(|p| {
                if p.available {
                    format!("{} [OK]", p.name)
                } else {
                    format!("{} [NO]", p.name)
                }
            })
            .collect();
        format!("Tools: {}", parts.join(" "))
    };
    lines.push(tool_summary);

    lines.join("\n")
}

fn build_tool_probes() -> Vec<ToolProbeEntry> {
    PROBE_LIST
        .iter()
        .map(|&(name, _)| ToolProbeEntry {
            name: name.to_string(),
            available: probe_cmd(name),
        })
        .collect()
}

// ── Platform dispatch ────────────────────────────────────────────────

fn os_version() -> String {
    if cfg!(target_os = "windows") {
        run_capture_hidden("cmd", &["/C", "ver"])
    } else {
        run_capture_hidden("uname", &["-r"])
    }
    .lines()
    .next()
    .unwrap_or("unknown")
    .trim()
    .to_string()
}

fn shell_info() -> String {
    if cfg!(target_os = "windows") {
        let ver = run_capture_hidden("cmd", &["/C", "ver"]);
        let ver = ver.lines().next().unwrap_or("").trim();
        let ps = run_capture_hidden(
            "powershell",
            &["-Command", "$PSVersionTable.PSVersion.ToString()"],
        );
        let ps = ps.trim();
        if ps.is_empty() {
            format!("Shell: cmd ({})", ver)
        } else {
            format!("Shell: cmd / PowerShell {}", ps)
        }
    } else {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let bash_ver = run_capture_hidden("bash", &["--version"]);
        let bash_ver = bash_ver.lines().next().unwrap_or("").trim();
        if bash_ver.is_empty() {
            format!("Shell: {}", sh)
        } else {
            format!("Shell: {} ({})", sh, bash_ver)
        }
    }
}

// ── Windows: raw Win32 FFI ──────────────────────────────────────────

#[cfg(target_os = "windows")]
mod win32 {
    use std::mem;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemInfo(lpSystemInfo: *mut SYSTEM_INFO);
        fn GlobalMemoryStatusEx(lpBuffer: *mut MemoryStatusEx) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            hKey: isize,
            lpSubKey: *const u16,
            ulOptions: u32,
            samDesired: u32,
            phkResult: *mut isize,
        ) -> i32;

        fn RegEnumKeyExW(
            hKey: isize,
            dwIndex: u32,
            lpName: *mut u16,
            lpcchName: *mut u32,
            lpReserved: *mut u32,
            lpClass: *mut u16,
            lpcchClass: *mut u32,
            lpftLastWriteTime: *mut u64,
        ) -> i32;

        fn RegQueryValueExW(
            hKey: isize,
            lpValueName: *const u16,
            lpReserved: *mut u32,
            lpType: *mut u32,
            lpData: *mut u8,
            lpcbData: *mut u32,
        ) -> i32;

        fn RegCloseKey(hKey: isize) -> i32;
    }

    const HKEY_LOCAL_MACHINE: isize = 0x80000002u32 as isize;
    const KEY_READ: u32 = 0x20019;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_NO_MORE_ITEMS: i32 = 259;

    #[repr(C)]
    struct SYSTEM_INFO {
        w_processor_architecture: u16,
        w_reserved: u16,
        dw_page_size: u32,
        lp_minimum_application_address: *mut u8,
        lp_maximum_application_address: *mut u8,
        dw_active_processor_mask: usize,
        dw_number_of_processors: u32,
        dw_processor_type: u32,
        dw_allocation_granularity: u32,
        w_processor_level: u16,
        w_processor_revision: u16,
    }

    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    fn reg_open(subkey: &str) -> Option<isize> {
        let wsub = wide(subkey);
        let mut hkey: isize = 0;
        let rc =
            unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, wsub.as_ptr(), 0, KEY_READ, &mut hkey) };
        if rc == ERROR_SUCCESS {
            Some(hkey)
        } else {
            None
        }
    }

    fn reg_get_string(hkey: isize, name: &str) -> Option<String> {
        let wname = wide(name);
        let mut buf_type: u32 = 0;
        let mut buf_len: u32 = 0;
        let rc = unsafe {
            RegQueryValueExW(
                hkey,
                wname.as_ptr(),
                std::ptr::null_mut(),
                &mut buf_type,
                std::ptr::null_mut(),
                &mut buf_len,
            )
        };
        if rc != ERROR_SUCCESS || buf_len == 0 {
            return None;
        }
        let mut data = vec![0u8; buf_len as usize];
        let rc = unsafe {
            RegQueryValueExW(
                hkey,
                wname.as_ptr(),
                std::ptr::null_mut(),
                &mut buf_type,
                data.as_mut_ptr(),
                &mut buf_len,
            )
        };
        if rc != ERROR_SUCCESS {
            return None;
        }
        let u16_len = buf_len as usize / 2;
        if u16_len == 0 {
            return None;
        }
        let wide_data: Vec<u16> = data[..u16_len * 2]
            .chunks_exact(2)
            .map(|c| u16::from_ne_bytes([c[0], c[1]]))
            .collect();
        Some(from_wide(&wide_data))
    }

    fn reg_enum_subkeys(hkey: isize) -> Vec<String> {
        let mut names = Vec::new();
        let mut index: u32 = 0;
        loop {
            let mut name_buf = [0u16; 256];
            let mut name_len: u32 = 256;
            let rc = unsafe {
                RegEnumKeyExW(
                    hkey,
                    index,
                    name_buf.as_mut_ptr(),
                    &mut name_len,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if rc == ERROR_NO_MORE_ITEMS {
                break;
            }
            if rc == ERROR_SUCCESS {
                names.push(from_wide(&name_buf[..name_len as usize]));
            }
            index += 1;
            if index > 4096 {
                break;
            }
        }
        names
    }

    fn reg_close(hkey: isize) {
        unsafe {
            RegCloseKey(hkey);
        }
    }

    pub fn cpu_info() -> String {
        let mut si: SYSTEM_INFO = unsafe { mem::zeroed() };
        unsafe {
            GetSystemInfo(&mut si);
        }
        let cores = si.dw_number_of_processors;
        let name = reg_open(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
            .and_then(|h| {
                let v = reg_get_string(h, "ProcessorNameString");
                reg_close(h);
                v
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            format!("CPU: {} cores", cores)
        } else {
            format!("CPU: {} ({} cores)", name, cores)
        }
    }

    pub fn memory_info() -> String {
        let mut ms: MemoryStatusEx = unsafe { mem::zeroed() };
        ms.dw_length = mem::size_of::<MemoryStatusEx>() as u32;
        let rc = unsafe { GlobalMemoryStatusEx(&mut ms) };
        if rc != 0 && ms.ull_total_phys > 0 {
            format!("RAM: {} GB", ms.ull_total_phys / 1_073_741_824)
        } else {
            "RAM: unknown".to_string()
        }
    }

    pub fn gpu_info() -> String {
        let pci = match reg_open(r"SYSTEM\CurrentControlSet\Enum\PCI") {
            Some(h) => h,
            None => return "GPU: unknown".to_string(),
        };
        let gpu_vendors = ["NVIDIA", "AMD", "Radeon", "Intel", "Arc"];
        let mut gpus: Vec<String> = Vec::new();
        for vendor_key in reg_enum_subkeys(pci) {
            let vendor_path = format!(r"SYSTEM\CurrentControlSet\Enum\PCI\{}", vendor_key);
            let hv = match reg_open(&vendor_path) {
                Some(h) => h,
                None => continue,
            };
            for device_key in reg_enum_subkeys(hv) {
                let device_path = format!(
                    r"SYSTEM\CurrentControlSet\Enum\PCI\{}\{}",
                    vendor_key, device_key
                );
                let hd = match reg_open(&device_path) {
                    Some(h) => h,
                    None => continue,
                };
                let raw_name = reg_get_string(hd, "FriendlyName")
                    .or_else(|| reg_get_string(hd, "DeviceDesc"))
                    .unwrap_or_default();
                reg_close(hd);
                let name = raw_name.trim().to_string();
                if name.is_empty() {
                    continue;
                }
                let display_name = name.rsplit(';').next().unwrap_or(&name).trim().to_string();
                let upper = display_name.to_ascii_uppercase();
                if gpu_vendors.iter().any(|v| upper.contains(v))
                    && !gpus.iter().any(|g| g == &display_name)
                {
                    gpus.push(display_name);
                }
            }
            reg_close(hv);
        }
        reg_close(pci);
        if gpus.is_empty() {
            "GPU: unknown".to_string()
        } else {
            format!("GPU: {}", gpus.join(", "))
        }
    }
}

#[cfg(target_os = "windows")]
fn cpu_info() -> String {
    win32::cpu_info()
}

#[cfg(target_os = "windows")]
fn memory_info() -> String {
    win32::memory_info()
}

#[cfg(target_os = "windows")]
fn gpu_info() -> String {
    win32::gpu_info()
}

#[cfg(not(target_os = "windows"))]
fn cpu_info() -> String {
    if cfg!(target_os = "macos") {
        let name = run_capture_hidden("sysctl", &["-n", "machdep.cpu.brand_string"]);
        let cores = run_capture_hidden("sysctl", &["-n", "hw.ncpu"]);
        format!("CPU: {} ({} cores)", name.trim(), cores.trim())
    } else {
        let name = grep_field("/proc/cpuinfo", "model name");
        let cores = grep_field("/proc/cpuinfo", "cpu cores");
        if name.is_empty() {
            let nproc = run_capture_hidden("nproc", &[]);
            format!("CPU: {} cores", nproc.trim())
        } else {
            format!(
                "CPU: {} ({} cores)",
                name,
                if cores.is_empty() { "?" } else { &cores }
            )
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn memory_info() -> String {
    if cfg!(target_os = "macos") {
        let out = run_capture_hidden("sysctl", &["-n", "hw.memsize"]);
        let bytes: u64 = out.trim().parse().unwrap_or(0);
        if bytes > 0 {
            format!("RAM: {} GB", bytes / 1_073_741_824)
        } else {
            "RAM: unknown".to_string()
        }
    } else {
        let out = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let total_kb = out
            .lines()
            .find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        if total_kb > 0 {
            format!("RAM: {} GB", total_kb / 1_048_576)
        } else {
            "RAM: unknown".to_string()
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn gpu_info() -> String {
    if cfg!(target_os = "macos") {
        let out = run_capture_hidden(
            "system_profiler",
            &["SPDisplaysDataType", "-detaillevel", "mini"],
        );
        let chip = out
            .lines()
            .find(|l| l.contains("Chipset Model") || l.contains("Metal"))
            .unwrap_or("")
            .split(':')
            .nth(1)
            .unwrap_or("unknown")
            .trim()
            .to_string();
        if chip.is_empty() || chip == "unknown" {
            "GPU: unknown".to_string()
        } else {
            format!("GPU: {}", chip)
        }
    } else {
        linux_gpu_info()
    }
}

#[cfg(not(target_os = "windows"))]
fn linux_gpu_info() -> String {
    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        let names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let fname = path.file_name()?.to_string_lossy().into_owned();
                if !fname.starts_with("card") || fname.contains('-') {
                    return None;
                }
                let product = path.join("device/product_name");
                if let Ok(name) = std::fs::read_to_string(&product) {
                    let n = name.trim().to_string();
                    if !n.is_empty() {
                        return Some(n);
                    }
                }
                let label = path.join("device/label");
                if let Ok(name) = std::fs::read_to_string(&label) {
                    let n = name.trim().to_string();
                    if !n.is_empty() {
                        return Some(n);
                    }
                }
                None
            })
            .collect();
        if !names.is_empty() {
            return format!("GPU: {}", names.join(", "));
        }
    }
    if let Ok(entries) = std::fs::read_dir("/proc/driver/nvidia/gpus") {
        let names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let info = std::fs::read_to_string(e.path().join("information")).ok()?;
                info.lines()
                    .find(|l| l.starts_with("Model:"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|v| v.trim().to_string())
            })
            .collect();
        if !names.is_empty() {
            return format!("GPU: {}", names.join(", "));
        }
    }
    let out = run_capture_hidden("lspci", &["-mm"]);
    let gpu_lines: Vec<&str> = out
        .lines()
        .filter(|l| {
            let lo = l.to_ascii_lowercase();
            lo.contains("vga") || lo.contains("3d") || lo.contains("display")
        })
        .collect();
    if !gpu_lines.is_empty() {
        let names: Vec<String> = gpu_lines
            .iter()
            .map(|l| {
                let vendor = l.split('"').nth(3).unwrap_or("").trim();
                let device = l.split('"').nth(5).unwrap_or("").trim();
                if vendor.is_empty() && device.is_empty() {
                    l.trim().to_string()
                } else if vendor.is_empty() {
                    device.to_string()
                } else {
                    format!("{} {}", vendor, device)
                }
            })
            .collect();
        return format!("GPU: {}", names.join(", "));
    }
    "GPU: unknown".to_string()
}

// ── Tool probing ─────────────────────────────────────────────────────

const PROBE_LIST: &[(&str, &str)] = &[
    ("rg", "ripgrep"),
    ("grep", "GNU grep"),
    ("git", "version control"),
    ("curl", "HTTP client"),
    ("python", "Python interpreter"),
    ("python3", "Python 3"),
    ("node", "Node.js"),
    ("cargo", "Rust build"),
    ("npm", "Node package manager"),
    ("pip", "Python package manager"),
    ("make", "Build automation"),
    ("docker", "Container runtime"),
    ("findstr", "Windows text search"),
    ("powershell", "PowerShell"),
];

fn probe_cmd(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        run_hidden("where", &[name])
    } else {
        run_hidden("which", &[name])
    }
}

// ── Hidden subprocess helpers ────────────────────────────────────────
// On Windows, CREATE_NO_WINDOW (0x08000000) prevents console flash.

#[cfg(target_os = "windows")]
fn run_hidden(cmd: &str, args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new(cmd)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn run_capture_hidden(cmd: &str, args: &[&str]) -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new(cmd)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
fn run_hidden(cmd: &str, args: &[&str]) -> bool {
    std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn run_capture_hidden(cmd: &str, args: &[&str]) -> String {
    std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
fn grep_field(path: &str, field: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.split(':').nth(1))
        .map(|v| v.trim().to_string())
        .unwrap_or_default()
}

// ── Runtime helpers ───────────────────────────────────────────────────

pub fn grep_note() -> &'static str {
    LIVE_CACHE.get().map(|i| grep_note_from(i)).unwrap_or("")
}

pub fn shell_tools_note() -> String {
    LIVE_CACHE
        .get()
        .map(shell_tools_note_from)
        .unwrap_or_default()
}

/// Returns true if the system has a usable OpenGL library available.
pub fn has_opengl() -> bool {
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        let known_paths = [
            "/usr/lib/libGL.so.1",
            "/usr/lib/libGL.so",
            "/usr/lib/x86_64-linux-gnu/libGL.so.1",
            "/usr/lib/aarch64-linux-gnu/libGL.so.1",
            "/usr/lib/i386-linux-gnu/libGL.so.1",
            "/usr/lib32/libGL.so.1",
            "/usr/lib64/libGL.so.1",
        ];
        if known_paths.iter().any(|p| std::path::Path::new(p).exists()) {
            return true;
        }
        if let Ok(output) = std::process::Command::new("ldconfig").arg("-p").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.lines().any(|l| l.contains("libGL.so")) {
                return true;
            }
        }
        false
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        false
    }
}
