#[cfg(target_os = "linux")]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::manifest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drive {
    pub device: String,
    pub vendor: String,
    pub product: String,
    pub revision: String,
}

impl From<&Drive> for manifest::DriveMatch {
    fn from(d: &Drive) -> Self {
        manifest::DriveMatch {
            vendor: d.vendor.clone(),
            model: d.product.clone(),
            revision: d.revision.clone(),
        }
    }
}

/// Parse drive identity from sdftool `--info` output for manifest matching.
pub fn parse_identity_from_info(device: &str, info_output: &str) -> manifest::DriveMatch {
    let mut vendor = String::new();
    let mut model = String::new();
    let mut revision = String::new();

    for line in info_output.lines() {
        let line = line.trim();
        if let Some(val) = line
            .strip_prefix("Vendor:")
            .or_else(|| line.strip_prefix("vendor:"))
        {
            vendor = val.trim().to_string();
        } else if let Some(val) = line
            .strip_prefix("Product:")
            .or_else(|| line.strip_prefix("product:"))
            .or_else(|| line.strip_prefix("Model:"))
            .or_else(|| line.strip_prefix("model:"))
        {
            model = val.trim().to_string();
        } else if let Some(val) = line
            .strip_prefix("Revision:")
            .or_else(|| line.strip_prefix("revision:"))
            .or_else(|| line.strip_prefix("Firmware:"))
            .or_else(|| line.strip_prefix("firmware:"))
        {
            revision = val.trim().to_string();
        }
    }

    if vendor.is_empty() && model.is_empty() {
        if let Some((v, m)) = device.split_once('_') {
            if !v.is_empty() {
                vendor = v.to_string();
            }
            if !m.is_empty() {
                model = m.to_string();
            }
        }
    }

    manifest::DriveMatch {
        vendor,
        model,
        revision,
    }
}

/// DriveMatch for manifest gates: probe identity wins when present.
pub fn drive_match_for_validation(
    drive: &Drive,
    probe_identity: Option<&manifest::DriveMatch>,
) -> manifest::DriveMatch {
    let base: manifest::DriveMatch = drive.into();
    let Some(probe) = probe_identity else {
        return base;
    };
    manifest::DriveMatch {
        vendor: if probe.vendor.is_empty() {
            base.vendor
        } else {
            probe.vendor.clone()
        },
        model: if probe.model.is_empty() {
            base.model
        } else {
            probe.model.clone()
        },
        revision: if probe.revision.is_empty() {
            base.revision
        } else {
            probe.revision.clone()
        },
    }
}

/// Parse `sdftool -l` stdout into drives.
pub fn parse_drive_list(output: &str) -> Vec<Drive> {
    let mut drives = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit() || c == ':') {
            let rest = rest.trim_start_matches(':').trim();
            if !rest.is_empty() && (rest.starts_with("/dev/") || rest.contains(':')) {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                let device = parts[0].to_string();
                let vendor = parts.get(1).copied().unwrap_or("").to_string();
                let (product, revision) = match parts.len() {
                    n if n >= 5 => (parts[2..n - 1].join(" "), parts[n - 1].to_string()),
                    4 => (parts[2].to_string(), parts[3].to_string()),
                    3 => (parts[2].to_string(), String::new()),
                    _ => (String::new(), String::new()),
                };
                drives.push(Drive {
                    device,
                    vendor,
                    product,
                    revision,
                });
            }
        }
    }
    drives
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
    fn backend_from_name_sdftool() {
        assert_eq!(
            backend_from_name("sdftool"),
            crate::command::Backend::SdfTool
        );
        assert_eq!(
            backend_from_name("sdftool64"),
            crate::command::Backend::SdfTool
        );
        assert_eq!(
            backend_from_name("/usr/bin/sdftool"),
            crate::command::Backend::SdfTool
        );
    }

    #[test]
    fn backend_from_name_makemkvcon() {
        assert_eq!(
            backend_from_name("makemkvcon"),
            crate::command::Backend::MakeMkvCon
        );
        assert_eq!(
            backend_from_name("makemkvcon64"),
            crate::command::Backend::MakeMkvCon
        );
        assert_eq!(
            backend_from_name("makemkv_something"),
            crate::command::Backend::MakeMkvCon
        );
    }

    #[test]
    fn backend_from_name_unknown() {
        assert_eq!(
            backend_from_name("unknown_tool"),
            crate::command::Backend::SdfTool
        );
        assert_eq!(backend_from_name(""), crate::command::Backend::SdfTool);
    }

    #[test]
    fn resolve_name_existing_file() {
        let dir = std::env::temp_dir().join("sdf_flash_test_resolve");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("test_binary");
        std::fs::write(&file, b"").unwrap();
        let name = resolve_name(&file.to_string_lossy());
        assert_eq!(name, "test_binary");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_name_nonexistent() {
        let name = resolve_name("/nonexistent/path/binary");
        assert_eq!(name, "binary");
    }

    #[test]
    fn drive_to_drive_match() {
        let drive = Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
        };
        let dm: manifest::DriveMatch = (&drive).into();
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BU40N");
        assert_eq!(dm.revision, "1.03");
    }

    #[test]
    fn drive_to_drive_match_empty() {
        let drive = Drive {
            device: "/dev/sr0".into(),
            vendor: String::new(),
            product: String::new(),
            revision: String::new(),
        };
        let dm: manifest::DriveMatch = (&drive).into();
        assert!(dm.vendor.is_empty());
        assert!(dm.model.is_empty());
        assert!(dm.revision.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn parse_vendor_product_underscore() {
        let (v, p) = parse_vendor_product("HL-DT-ST_BU40N");
        assert_eq!(v, "HL-DT-ST");
        assert_eq!(p, "BU40N");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn parse_vendor_product_space() {
        let (v, p) = parse_vendor_product("HL-DT-ST BU40N");
        assert_eq!(v, "HL-DT-ST");
        assert_eq!(p, "BU40N");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn parse_vendor_product_no_separator() {
        let (v, p) = parse_vendor_product("BU40N");
        assert!(v.is_empty());
        assert_eq!(p, "BU40N");
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

    #[test]
    fn parse_drive_list_four_fields() {
        let output = "0:/dev/sr0 HL-DT-ST BU40N 1.03\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[0].product, "BU40N");
        assert_eq!(drives[0].revision, "1.03");
    }

    #[test]
    fn parse_drive_list_five_fields() {
        let output = "0:/dev/sr0 HL-DT-ST BD-RE BU40N 1.03\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].product, "BD-RE BU40N");
        assert_eq!(drives[0].revision, "1.03");
    }

    #[test]
    fn drive_match_prefers_probe() {
        let drive = Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
        };
        let probe = manifest::DriveMatch {
            vendor: "HL-DT-ST".into(),
            model: "BD-RE BU40N".into(),
            revision: "1.03".into(),
        };
        let dm = drive_match_for_validation(&drive, Some(&probe));
        assert_eq!(dm.model, "BD-RE BU40N");
    }
}
