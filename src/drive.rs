// Optical drive enumeration — cross-platform.

use std::io::{BufRead, BufReader};
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
//
// macOS:  IOKit (via core-foundation-sys raw bindings)
// Linux:  sysfs (/dev/sr* + /sys/block/sr*/device/)
// Windows: drive letters + IOCTL

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drive {
    pub device: String,
    pub vendor: String,
    pub product: String,
    pub revision: String,
}

/// Enumerate all optical drives on the system.
pub fn enumerate_drives() -> Vec<Drive> {
    #[cfg(target_os = "macos")]
    {
        enumerate_macos()
    }

    #[cfg(target_os = "linux")]
    {
        enumerate_linux()
    }

    #[cfg(target_os = "windows")]
    {
        enumerate_windows()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// Run an external command and return stdout+stderr.
pub fn run_command(program: &str, args: &[String]) -> Result<CommandOutput, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    Ok(CommandOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Extract a 0–100 progress value from a tool output line (MakeMKV PRGV, `NN%`, etc.).
pub fn parse_progress_percent(line: &str) -> Option<f32> {
    let line = line.trim();

    if let Some(rest) = line.strip_prefix("PRGV:") {
        let mut parts = rest.split(',');
        let current: f32 = parts.next()?.parse().ok()?;
        let total: f32 = parts.next()?.parse().ok()?;
        if total > 0.0 {
            return Some((current / total * 100.0).clamp(0.0, 100.0));
        }
    }

    if let Some(idx) = line.rfind('%') {
        let before = &line[..idx];
        let digits: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if let Ok(n) = digits.parse::<f32>() {
            return Some(n.clamp(0.0, 100.0));
        }
    }

    None
}

/// Run a command, invoking `on_line` for each stdout/stderr line as it arrives.
pub fn run_command_streaming<F>(
    program: &str,
    args: &[String],
    mut on_line: F,
) -> Result<CommandOutput, String>
where
    F: FnMut(&str),
{
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture stderr".to_string())?;

    enum PipeLine {
        Out(String),
        Err(String),
    }

    let (tx, rx) = mpsc::channel::<PipeLine>();
    let tx_out = tx.clone();

    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_out.send(PipeLine::Out(line));
        }
    });

    let tx_err = tx.clone();
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_err.send(PipeLine::Err(line));
        }
    });

    drop(tx);

    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();
    while let Ok(msg) = rx.recv() {
        let line = match msg {
            PipeLine::Out(line) => {
                if !stdout_buf.is_empty() {
                    stdout_buf.push('\n');
                }
                stdout_buf.push_str(&line);
                line
            }
            PipeLine::Err(line) => {
                if !stderr_buf.is_empty() {
                    stderr_buf.push('\n');
                }
                stderr_buf.push_str(&line);
                line
            }
        };
        on_line(&line);
    }

    stdout_handle
        .join()
        .map_err(|_| "stdout reader panicked".to_string())?;
    stderr_handle
        .join()
        .map_err(|_| "stderr reader panicked".to_string())?;

    let status = child.wait().map_err(|e| e.to_string())?;

    Ok(CommandOutput {
        status,
        stdout: stdout_buf,
        stderr: stderr_buf,
    })
}

#[cfg(test)]
mod progress_tests {
    use super::parse_progress_percent;

    #[test]
    fn parses_prgv() {
        assert_eq!(parse_progress_percent("PRGV:50,100,0"), Some(50.0));
        assert_eq!(parse_progress_percent("PRGV:100,100,0"), Some(100.0));
    }

    #[test]
    fn parses_percent_suffix() {
        assert_eq!(parse_progress_percent("Progress: 42%"), Some(42.0));
        assert_eq!(
            parse_progress_percent("100% Operation finished"),
            Some(100.0)
        );
    }

    #[test]
    fn ignores_non_progress() {
        assert_eq!(parse_progress_percent("MSG:1005,0,2"), None);
    }
}

pub struct CommandOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn combined(&self) -> String {
        match (self.stdout.trim().is_empty(), self.stderr.trim().is_empty()) {
            (true, true) => String::new(),
            (false, true) => self.stdout.clone(),
            (true, false) => self.stderr.clone(),
            (false, false) => format!("{}\n{}", self.stdout.trim(), self.stderr.trim()),
        }
    }
}

// ---------------------------------------------------------------------------
// macOS implementation via IOKit
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn enumerate_macos() -> Vec<Drive> {
    // Use system_profiler or ioreg to find optical drives
    let Ok(output) = std::process::Command::new("system_profiler")
        .args(["SPSerialATADataType", "-json"])
        .output()
    else {
        return enumerate_macos_fallback();
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(items) = data.get("SPSerialATADataType").and_then(|v| v.as_array()) {
            let mut drives = Vec::new();
            for item in items {
                if let Some(devices) = item.get("_items").and_then(|v| v.as_array()) {
                    for dev in devices {
                        let name = dev
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let rev = dev
                            .get("revision_version")
                            .or_else(|| dev.get("bsd_name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let bsd = dev
                            .get("bsd_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !bsd.is_empty() {
                            // Parse vendor/product from name
                            let (vendor, product) = parse_vendor_product(&name);
                            drives.push(Drive {
                                device: format!("/dev/r{bsd}"),
                                vendor,
                                product,
                                revision: rev,
                            });
                        }
                    }
                }
            }
            if !drives.is_empty() {
                return drives;
            }
        }
    }

    enumerate_macos_fallback()
}

#[cfg(target_os = "macos")]
fn enumerate_macos_fallback() -> Vec<Drive> {
    // Fallback: scan /dev/rdisk* for optical drives
    let mut drives = Vec::new();
    for i in 0..8 {
        let device = format!("/dev/rdisk{i}");
        if std::path::Path::new(&device).exists() {
            // Try to identify via diskutil
            if let Ok(out) = std::process::Command::new("diskutil")
                .args(["info", &device])
                .output()
            {
                let info = String::from_utf8_lossy(&out.stdout);
                if info.contains("Optical") || info.contains("DVD") || info.contains("BD-RE") {
                    drives.push(Drive {
                        device,
                        vendor: String::new(),
                        product: String::new(),
                        revision: String::new(),
                    });
                }
            }
        }
    }
    drives
}

// ---------------------------------------------------------------------------
// Linux implementation via sysfs
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn enumerate_linux() -> Vec<Drive> {
    let mut drives = Vec::new();
    for i in 0..8 {
        let device = format!("/dev/sr{i}");
        if std::path::Path::new(&device).exists() {
            let sys_path = format!("/sys/block/sr{i}/device");
            let vendor = read_sysfs_attr(&sys_path, "vendor").unwrap_or_default();
            let model = read_sysfs_attr(&sys_path, "model").unwrap_or_default();
            let rev = read_sysfs_attr(&sys_path, "rev").unwrap_or_default();
            drives.push(Drive {
                device,
                vendor: vendor.trim().to_string(),
                product: model.trim().to_string(),
                revision: rev.trim().to_string(),
            });
        }
    }
    drives
}

#[cfg(target_os = "linux")]
fn read_sysfs_attr(base: &str, attr: &str) -> Option<String> {
    let path = PathBuf::from(base).join(attr);
    std::fs::read_to_string(path).ok()
}

// ---------------------------------------------------------------------------
// Windows implementation via drive letters
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod winapi {
    extern "system" {
        pub fn GetDriveTypeA(lpRootPathName: *const u8) -> u32;
    }
}

#[cfg(target_os = "windows")]
fn enumerate_windows() -> Vec<Drive> {
    let mut drives = Vec::new();
    for letter in b'C'..=b'Z' {
        let device = format!("{}:", letter as char);
        // GetDriveTypeA expects a null-terminated string pointer (LPCSTR)
        let path = format!("{}:\\\0", device);

        // SAFETY: We pass a valid pointer to a null-terminated ASCII string ("X:\0").
        // The return value of 5 corresponds to DRIVE_CDROM.
        let is_cdrom = unsafe { winapi::GetDriveTypeA(path.as_ptr()) == 5 };

        if is_cdrom {
            drives.push(Drive {
                device,
                vendor: String::new(),
                product: String::new(),
                revision: String::new(),
            });
        }
    }
    drives
}

#[cfg(target_os = "macos")]
fn parse_vendor_product(name: &str) -> (String, String) {
    // Try to split "VENDOR PRODUCT" or "VENDOR_MODEL"
    if let Some(idx) = name.find('_') {
        (name[..idx].to_string(), name[idx + 1..].to_string())
    } else if let Some(idx) = name.find(' ') {
        (name[..idx].to_string(), name[idx + 1..].to_string())
    } else {
        (String::new(), name.to_string())
    }
}

/// Resolve a path through symlinks, returning the canonical target name.
fn resolve_name(path: &str) -> String {
    std::fs::canonicalize(path)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| {
            std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// Determine backend type from a resolved binary name.
fn backend_from_name(name: &str) -> super::command::Backend {
    if name.starts_with("makemkv") {
        super::command::Backend::MakeMkvCon
    } else {
        super::command::Backend::SdfTool
    }
}

/// Try to find sdftool or makemkvcon on the system.
pub fn find_backend() -> Option<(super::command::Backend, String)> {
    // Check common names via PATH lookup.
    // On Linux, sdftool is often a symlink to makemkvcon, so resolve
    // the symlink target before determining the backend type.
    for name in &["sdftool64", "sdftool", "makemkvcon64", "makemkvcon"] {
        if let Ok(path) = which(name) {
            let resolved = resolve_name(&path);
            let backend = backend_from_name(&resolved);
            return Some((backend, path));
        }
    }

    // Check common installation paths
    #[cfg(target_os = "macos")]
    {
        let paths = [
            "/opt/homebrew/bin/sdftool",
            "/usr/local/bin/sdftool",
            "/opt/homebrew/bin/makemkvcon",
            "/usr/local/bin/makemkvcon",
            "/Applications/MakeMKV.app/Contents/MacOS/sdftool",
            "/Applications/MakeMKV.app/Contents/MacOS/makemkvcon",
        ];
        for p in &paths {
            if std::path::Path::new(p).exists() {
                let resolved = resolve_name(p);
                return Some((backend_from_name(&resolved), p.to_string()));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let paths = [
            "/usr/bin/sdftool",
            "/usr/local/bin/sdftool",
            "/usr/bin/makemkvcon",
            "/usr/local/bin/makemkvcon",
            "/opt/makemkv/bin/makemkvcon",
        ];
        for p in &paths {
            if std::path::Path::new(p).exists() {
                let resolved = resolve_name(p);
                return Some((backend_from_name(&resolved), p.to_string()));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let paths = [
            r"C:\Program Files (x86)\MakeMKV\sdftool64.exe",
            r"C:\Program Files\MakeMKV\sdftool64.exe",
            r"C:\Program Files (x86)\MakeMKV\makemkvcon64.exe",
            r"C:\Program Files\MakeMKV\makemkvcon64.exe",
        ];
        for p in &paths {
            if std::path::Path::new(p).exists() {
                let resolved = resolve_name(p);
                return Some((backend_from_name(&resolved), p.to_string()));
            }
        }
    }

    None
}

fn which(name: &str) -> Result<String, String> {
    let output = std::process::Command::new(if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    })
    .arg(name)
    .output()
    .map_err(|e| e.to_string())?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }
    Err(format!("{name} not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prgv_format() {
        assert_eq!(parse_progress_percent("PRGV:50,100"), Some(50.0));
        assert_eq!(parse_progress_percent("PRGV:0,100"), Some(0.0));
        assert_eq!(parse_progress_percent("PRGV:100,100"), Some(100.0));
    }

    #[test]
    fn prgv_format_partial() {
        let p = parse_progress_percent("PRGV:33,99").unwrap();
        assert!((p - 33.33).abs() < 0.1);
    }

    #[test]
    fn percent_format() {
        assert_eq!(parse_progress_percent("42%"), Some(42.0));
        assert_eq!(parse_progress_percent("Progress: 75%"), Some(75.0));
        assert_eq!(parse_progress_percent("100%"), Some(100.0));
    }

    #[test]
    fn no_progress() {
        assert_eq!(parse_progress_percent("some random text"), None);
        assert_eq!(parse_progress_percent(""), None);
    }

    #[test]
    fn prgv_zero_total_clamps() {
        // PRGV with zero total should not produce NaN/Inf, clamp handles it
        assert_eq!(parse_progress_percent("PRGV:0,0"), None);
    }
}
