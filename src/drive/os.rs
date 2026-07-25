//! OS drive enumeration and backend/sdf.bin discovery.
//! Coverage-ignored (see scripts/coverage-ignore.regex).

use super::parse::{cap_drive_list, Drive, MAX_OPTICAL_DRIVES};
#[cfg(target_os = "macos")]
use super::parse::{parse_drutil_list, parse_ioreg_optical_services, parse_vendor_product};
use crate::command::Backend;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

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

// ---------------------------------------------------------------------------
// macOS (specs/04-scsi-transport, 34-backup-profile-drive)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn enumerate_macos() -> Vec<Drive> {
    let from_ioreg = enumerate_macos_ioreg();
    if !from_ioreg.is_empty() {
        return cap_drive_list(from_ioreg);
    }

    let from_drutil = enumerate_macos_drutil();
    if !from_drutil.is_empty() {
        return cap_drive_list(from_drutil);
    }

    if let Ok(output) = std::process::Command::new("system_profiler")
        .args(["SPSerialATADataType", "-json"])
        .output()
    {
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
                                let (vendor, product) = parse_vendor_product(&name);
                                drives.push(Drive {
                                    device: format!("/dev/r{bsd}"),
                                    vendor,
                                    product,
                                    revision: rev,
                                    ..Default::default()
                                });
                            }
                        }
                    }
                }
                if !drives.is_empty() {
                    return cap_drive_list(drives);
                }
            }
        }
    }

    cap_drive_list(enumerate_macos_diskutil_fallback())
}

#[cfg(target_os = "macos")]
fn enumerate_macos_ioreg() -> Vec<Drive> {
    const CLASSES: &[&str] = &["IOBDServices", "IODVDServices", "IOCompactDiscServices"];
    let mut drives = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for class in CLASSES {
        let Ok(output) = std::process::Command::new("ioreg")
            .args(["-r", "-c", class, "-l"])
            .output()
        else {
            continue;
        };
        if !output.status.success() && output.stdout.is_empty() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for d in parse_ioreg_optical_services(&text, class) {
            let key = d.identity_key();
            if seen.insert(key) {
                drives.push(d);
            }
            if drives.len() >= MAX_OPTICAL_DRIVES {
                return drives;
            }
        }
    }
    drives
}

#[cfg(target_os = "macos")]
fn enumerate_macos_drutil() -> Vec<Drive> {
    let Ok(output) = std::process::Command::new("drutil").arg("list").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_drutil_list(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn enumerate_macos_diskutil_fallback() -> Vec<Drive> {
    let mut drives = Vec::new();
    for i in 0..16 {
        let device = format!("/dev/rdisk{i}");
        if std::path::Path::new(&device).exists() {
            if let Ok(out) = std::process::Command::new("diskutil")
                .args(["info", &device])
                .output()
            {
                let info = String::from_utf8_lossy(&out.stdout);
                if info.contains("Optical")
                    || info.contains("DVD")
                    || info.contains("BD-RE")
                    || info.contains("CD-ROM")
                {
                    drives.push(Drive {
                        device,
                        vendor: String::new(),
                        product: String::new(),
                        revision: String::new(),
                        ..Default::default()
                    });
                }
            }
        }
    }
    drives
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn sort_by_index(names: &mut [String], prefix: &str) {
    names.sort_by_key(|n| {
        n.strip_prefix(prefix)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });
}

/// Collect `srN` block names from sysfs (handles USB-attached sr10+, etc.).
#[cfg(target_os = "linux")]
fn linux_sr_block_names() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(rest) = name.strip_prefix("sr") {
                if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                    names.push(name);
                }
            }
        }
    }
    sort_by_index(&mut names, "sr");
    if names.is_empty() {
        for i in 0..16 {
            let name = format!("sr{i}");
            if PathBuf::from(format!("/dev/{name}")).exists() {
                names.push(name);
            }
        }
    }
    names
}

/// SCSI device type 5 = MMC (CD/DVD/BD) per MakeMKV specs/04-scsi-transport.
#[cfg(target_os = "linux")]
const SCSI_TYPE_MMC: &str = "5";

/// Enumerate `/dev/sg*` optical devices via `/sys/class/scsi_generic/` (type 5).
/// MakeMKV/libdriveio prefers SG_IO on Linux (specs/34-backup-profile-drive §3.2).
#[cfg(target_os = "linux")]
fn enumerate_linux_sg() -> Vec<Drive> {
    let mut drives = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/scsi_generic") else {
        return drives;
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("sg"))
        .collect();
    sort_by_index(&mut names, "sg");
    for name in names {
        let type_path = format!("/sys/class/scsi_generic/{name}/device/type");
        let Ok(ty) = std::fs::read_to_string(&type_path) else {
            continue;
        };
        if ty.trim() != SCSI_TYPE_MMC {
            continue;
        }
        let sys_path = format!("/sys/class/scsi_generic/{name}/device");
        let vendor = read_sysfs_attr(&sys_path, "vendor").unwrap_or_default();
        let model = read_sysfs_attr(&sys_path, "model").unwrap_or_default();
        let rev = read_sysfs_attr(&sys_path, "rev").unwrap_or_default();
        let device = format!("/dev/{name}");
        if !std::path::Path::new(&device).exists() {
            continue;
        }
        drives.push(Drive {
            device,
            vendor: vendor.trim().to_string(),
            product: model.trim().to_string(),
            revision: rev.trim().to_string(),
            ..Default::default()
        });
        if drives.len() >= MAX_OPTICAL_DRIVES {
            break;
        }
    }
    drives
}

#[cfg(target_os = "linux")]
fn enumerate_linux_sr() -> Vec<Drive> {
    let mut drives = Vec::new();
    for name in linux_sr_block_names() {
        let device = format!("/dev/{name}");
        if !std::path::Path::new(&device).exists() {
            continue;
        }
        let sys_path = format!("/sys/block/{name}/device");
        let vendor = read_sysfs_attr(&sys_path, "vendor").unwrap_or_default();
        let model = read_sysfs_attr(&sys_path, "model").unwrap_or_default();
        let rev = read_sysfs_attr(&sys_path, "rev").unwrap_or_default();
        drives.push(Drive {
            device,
            vendor: vendor.trim().to_string(),
            product: model.trim().to_string(),
            revision: rev.trim().to_string(),
            ..Default::default()
        });
        if drives.len() >= MAX_OPTICAL_DRIVES {
            break;
        }
    }
    drives
}

/// Prefer `/dev/sg*` (SG_IO) over `/dev/sr*` when both expose the same identity.
#[cfg(target_os = "linux")]
fn merge_linux_drives(sg: Vec<Drive>, sr: Vec<Drive>) -> Vec<Drive> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for d in sg.into_iter().chain(sr) {
        let key = d.identity_key();
        let dedupe_key = if key == "||" {
            format!("dev:{}", d.device)
        } else {
            key
        };
        if seen.insert(dedupe_key) {
            out.push(d);
        }
        if out.len() >= MAX_OPTICAL_DRIVES {
            break;
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn enumerate_linux() -> Vec<Drive> {
    cap_drive_list(merge_linux_drives(
        enumerate_linux_sg(),
        enumerate_linux_sr(),
    ))
}

#[cfg(target_os = "linux")]
fn read_sysfs_attr(base: &str, attr: &str) -> Option<String> {
    let path = PathBuf::from(base).join(attr);
    std::fs::read_to_string(path).ok()
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod winapi {
    extern "system" {
        pub fn GetDriveTypeA(lpRootPathName: *const u8) -> u32;
    }
}

/// Win32 `DRIVE_CDROM` (specs/04-scsi-transport / MakeMKV GetDriveType scan).
#[cfg(target_os = "windows")]
const DRIVE_CDROM: u32 = 5;

#[cfg(target_os = "windows")]
fn enumerate_windows() -> Vec<Drive> {
    let mut drives = Vec::new();
    for letter in b'A'..=b'Z' {
        if drives.len() >= MAX_OPTICAL_DRIVES {
            break;
        }
        let device = format!("{}:", letter as char);
        let path = format!("{}\\\0", device);

        // SAFETY: `path` is a null-terminated ASCII root (`X:\`).
        let is_cdrom = unsafe { winapi::GetDriveTypeA(path.as_ptr()) == DRIVE_CDROM };

        if is_cdrom {
            drives.push(Drive {
                device,
                vendor: String::new(),
                product: String::new(),
                revision: String::new(),
                ..Default::default()
            });
        }
    }
    cap_drive_list(drives)
}

/// Try to find a backend binary on the system.
///
/// Searches for the `preferred` backend first (PATH + common install paths).
/// Falls back to the other backend if the preferred one is not found.
pub fn find_backend(preferred: Backend) -> Option<(Backend, String)> {
    for backend in [preferred, other_backend(preferred)] {
        if let Some(path) = find_for_backend(backend) {
            return Some((backend, path));
        }
    }
    None
}

pub(crate) fn other_backend(b: Backend) -> Backend {
    match b {
        Backend::SdfTool => Backend::MakeMkvCon,
        Backend::MakeMkvCon => Backend::SdfTool,
    }
}

/// Search PATH and common install paths for a specific backend's binary.
fn find_for_backend(backend: Backend) -> Option<String> {
    let names: &[&str] = match backend {
        Backend::SdfTool => &["sdftool64", "sdftool"],
        Backend::MakeMkvCon => &["makemkvcon64", "makemkvcon"],
    };
    for name in names {
        if let Ok(path) = which(name) {
            return Some(path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let paths: &[&str] = match backend {
            Backend::SdfTool => &[
                "/opt/homebrew/bin/sdftool",
                "/usr/local/bin/sdftool",
                "/Applications/MakeMKV.app/Contents/MacOS/sdftool",
            ],
            Backend::MakeMkvCon => &[
                "/opt/homebrew/bin/makemkvcon",
                "/usr/local/bin/makemkvcon",
                "/Applications/MakeMKV.app/Contents/MacOS/makemkvcon",
            ],
        };
        for p in paths {
            if std::path::Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let paths: &[&str] = match backend {
            Backend::SdfTool => &["/usr/bin/sdftool", "/usr/local/bin/sdftool"],
            Backend::MakeMkvCon => &[
                "/usr/bin/makemkvcon",
                "/usr/local/bin/makemkvcon",
                "/opt/makemkv/bin/makemkvcon",
            ],
        };
        for p in paths {
            if std::path::Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let paths: &[&str] = match backend {
            Backend::SdfTool => &[
                r"C:\Program Files (x86)\MakeMKV\sdftool64.exe",
                r"C:\Program Files\MakeMKV\sdftool64.exe",
            ],
            Backend::MakeMkvCon => &[
                r"C:\Program Files (x86)\MakeMKV\makemkvcon64.exe",
                r"C:\Program Files\MakeMKV\makemkvcon64.exe",
            ],
        };
        for p in paths {
            if std::path::Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
    }

    None
}

/// Locate `sdf.bin` in common relative and install paths.
pub fn find_sdf_bin() -> String {
    let candidates = ["./sdf.bin", "../sdf.bin", "/usr/share/sdftool/sdf.bin"];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.to_string();
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let paths = [
            format!("{home}/.MakeMKV/sdf.bin"),
            "/Library/MakeMKV/sdf.bin".to_string(),
            "/opt/homebrew/share/sdftool/sdf.bin".to_string(),
        ];
        for p in &paths {
            if std::path::Path::new(p).exists() {
                return p.clone();
            }
        }
    }

    String::new()
}

pub(crate) fn which(name: &str) -> Result<String, String> {
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
    fn other_backend_swaps() {
        assert_eq!(
            other_backend(crate::command::Backend::SdfTool),
            crate::command::Backend::MakeMkvCon
        );
        assert_eq!(
            other_backend(crate::command::Backend::MakeMkvCon),
            crate::command::Backend::SdfTool
        );
    }

    #[test]
    fn find_backend_prefers_selected() {
        for pref in [
            crate::command::Backend::SdfTool,
            crate::command::Backend::MakeMkvCon,
        ] {
            if let Some((backend, path)) = find_backend(pref) {
                assert!(!path.is_empty());
                let name = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let expect = match backend {
                    crate::command::Backend::SdfTool => "sdftool",
                    crate::command::Backend::MakeMkvCon => "makemkv",
                };
                assert!(name.contains(expect));
            }
        }
    }

    #[test]
    fn which_returns_error_for_nonexistent() {
        let result = which("definitely_not_a_real_command_12345");
        assert!(result.is_err());
    }

    #[test]
    fn which_finds_echo() {
        let result = which("echo");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("echo"));
    }
}
