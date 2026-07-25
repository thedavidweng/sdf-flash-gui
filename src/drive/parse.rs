//! Drive model, list parsing, and selection helpers (pure / no OS I/O).

use serde::{Deserialize, Serialize};

/// MakeMKV `AP_MaxCdromDevices` — hard cap for list UIs and OS enumeration.
/// See `makemkvgui/inc/lgpl/apdefs.h` and specs/33-robot-profile-complete-spec.md.
pub const MAX_OPTICAL_DRIVES: usize = 16;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Drive {
    /// OS / MakeMKV device path used with `sdftool -d` (e.g. `/dev/sr0`,
    /// `E:`, `/IOBDServices/…`, or a BuildDriveId string).
    pub device: String,
    pub vendor: String,
    pub product: String,
    pub revision: String,
    /// Serial from MakeMKV BuildDriveId tail (e.g. `MODJ9TK3546`).
    #[serde(default)]
    pub serial: String,
    /// Compact firmware date from BuildDriveId (`211904231648` = 2119-04-23 16:48).
    #[serde(default)]
    pub firmware_date: String,
}

impl Drive {
    /// Stable fingerprint for re-selection after bus re-enumeration
    /// (flash/reset may change `/dev/sgN` or IOKit path — MakeMKV § firmware protocol).
    pub fn identity_key(&self) -> String {
        format!(
            "{}|{}|{}",
            self.vendor.trim(),
            self.product.trim(),
            self.revision.trim()
        )
    }

    /// MakeMKV `BuildDriveId` style (`scsihlp.cpp:403`):  
    /// `Vendor_Product_Revision` with spaces → `_`, non-printable skipped.
    pub fn build_drive_id(&self) -> String {
        let mut s = String::new();
        append_makemkv_id_field(&mut s, &self.vendor);
        s.push('_');
        append_makemkv_id_field(&mut s, &self.product);
        s.push('_');
        append_makemkv_id_field(&mut s, &self.revision);
        s
    }

    /// Human-readable firmware date (`2119-04-23 16:48`), or empty.
    pub fn firmware_date_display(&self) -> String {
        format_firmware_date_raw(&self.firmware_date)
    }
}

/// Format compact MakeMKV firmware date token for UI.
///
/// `211904231648` → `2119-04-23 16:48`; `21190423` → `2119-04-23`.
pub fn format_firmware_date_raw(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() >= 12 && raw.as_bytes().iter().all(u8::is_ascii_digit) {
        format!(
            "{}-{}-{} {}:{}",
            &raw[0..4],
            &raw[4..6],
            &raw[6..8],
            &raw[8..10],
            &raw[10..12]
        )
    } else if raw.len() >= 8 && raw.as_bytes().iter().all(u8::is_ascii_digit) {
        format!("{}-{}-{}", &raw[0..4], &raw[4..6], &raw[6..8])
    } else {
        raw.to_string()
    }
}

/// True when token looks like a firmware calendar stamp (`211904231648` / `21200507`).
fn looks_like_firmware_date_token(s: &str) -> bool {
    (s.len() == 8 || s.len() == 12) && s.as_bytes().iter().all(u8::is_ascii_digit)
}

/// Append a field the way libdriveio `AppendStr` does (printable ASCII, space→`_`).
fn append_makemkv_id_field(dst: &mut String, src: &str) {
    for c in src.chars() {
        let u = c as u32;
        if !(0x20..=0x7e).contains(&u) {
            continue;
        }
        if c == ' ' {
            dst.push('_');
        } else {
            dst.push(c);
        }
    }
}

/// Re-select a drive after re-enumeration.
///
/// Order (aligned with MakeMKV stability needs after flash reset):
/// 1. Exact device path match  
/// 2. Identity key (`vendor|product|revision`)  
/// 3. Previous index if still in range  
/// 4. First drive (`Some(0)`) when the list is non-empty  
pub fn resolve_selection(
    drives: &[Drive],
    previous: Option<&Drive>,
    previous_index: Option<usize>,
) -> Option<usize> {
    if drives.is_empty() {
        return None;
    }
    if let Some(prev) = previous {
        if !prev.device.is_empty() {
            if let Some(i) = drives.iter().position(|d| d.device == prev.device) {
                return Some(i);
            }
        }
        let key = prev.identity_key();
        if key != "||" {
            if let Some(i) = drives.iter().position(|d| d.identity_key() == key) {
                return Some(i);
            }
        }
    }
    if let Some(i) = previous_index {
        if i < drives.len() {
            return Some(i);
        }
    }
    Some(0)
}

/// Cap a drive list at [`MAX_OPTICAL_DRIVES`] (MakeMKV AP_MaxCdromDevices).
pub fn cap_drive_list(mut drives: Vec<Drive>) -> Vec<Drive> {
    if drives.len() > MAX_OPTICAL_DRIVES {
        drives.truncate(MAX_OPTICAL_DRIVES);
    }
    drives
}

/// Drive identity (vendor / model / revision) used for validation.
///
/// Replaces the former `manifest::DriveMatch` — a standalone struct with the
/// same fields so probe/identity logic is independent of the manifest system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveIdentity {
    pub vendor: String,
    pub model: String,
    pub revision: String,
}

impl From<&Drive> for DriveIdentity {
    fn from(d: &Drive) -> Self {
        DriveIdentity {
            vendor: d.vendor.clone(),
            model: d.product.clone(),
            revision: d.revision.clone(),
        }
    }
}

/// Parse drive identity from sdftool `--info` output.
pub fn parse_identity_from_info(device: &str, info_output: &str) -> DriveIdentity {
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

    DriveIdentity {
        vendor,
        model,
        revision,
    }
}

/// True when `token` is a Windows drive letter (`D:` / `e:`).
fn is_windows_drive_letter(token: &str) -> bool {
    let b = token.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// True when `token` looks like a backend device path / drive letter.
///
/// Accepts:
/// - Linux: `/dev/sr*`, `/dev/sg*`
/// - macOS: `/dev/rdisk*`, MakeMKV IOKit paths (`/IOBDServices/…`, …)
/// - Windows: `D:`, `\\.\D:`, `\\.\CdRom0`
fn is_drive_device_token(token: &str) -> bool {
    if token.starts_with("/dev/") || token.starts_with("/IO") {
        return true;
    }
    if is_windows_drive_letter(token) {
        return true;
    }
    if let Some(rest) = token.strip_prefix(r"\\.\") {
        if is_windows_drive_letter(rest) {
            return true;
        }
        if rest.len() > 5
            && rest
                .get(..5)
                .is_some_and(|p| p.eq_ignore_ascii_case("CdRom"))
            && rest[5..].chars().all(|c| c.is_ascii_digit())
        {
            return true;
        }
    }
    false
}

/// sdftool list second-line status that is not an identity string.
fn is_list_status_line(ident: &str) -> bool {
    matches!(ident, "open error" | "query error")
}

/// Split a `sdftool -l` index line: `"00: /path …"` → `Some(("/path …"))`.
///
/// The index may be one or more digits (`0:`, `00:`). Returns `None` when the
/// line is not an index entry.
fn split_drive_list_index_line(line: &str) -> Option<&str> {
    let line = line.trim();
    let mut i = 0;
    let bytes = line.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= bytes.len() || bytes[i] != b':' {
        return None;
    }
    Some(line[i + 1..].trim())
}

/// Firmware revision token in MakeMKV identity strings (`GE03`, `1.03`, `3.10`).
fn looks_like_firmware_rev(s: &str) -> bool {
    if s.len() < 2 || s.len() > 8 {
        return false;
    }
    let mut has_alnum = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            has_alnum = true;
        } else if c != '.' {
            return false;
        }
    }
    has_alnum
}

/// Parsed MakeMKV BuildDriveId / list identity line fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnderscoreIdentity {
    pub vendor: String,
    pub product: String,
    pub revision: String,
    /// Compact date token (`211904231648`), may be empty.
    pub firmware_date: String,
    pub serial: String,
}

impl UnderscoreIdentity {
    fn apply_to_drive(self, drive: &mut Drive) {
        if !self.vendor.is_empty() {
            drive.vendor = self.vendor;
        }
        if !self.product.is_empty() {
            drive.product = self.product;
        }
        if !self.revision.is_empty() {
            drive.revision = self.revision;
        }
        if !self.firmware_date.is_empty() {
            drive.firmware_date = self.firmware_date;
        }
        if !self.serial.is_empty() {
            drive.serial = self.serial;
        }
    }
}

/// Parse MakeMKV-style underscore identity:
/// `HL-DT-ST_BD-RE_BU50N_GE03_211904231648_MODJ9TK3546`
/// → vendor, product, revision, firmware date, serial.
pub fn parse_underscore_identity_full(ident: &str) -> UnderscoreIdentity {
    let parts: Vec<&str> = ident.split('_').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return UnderscoreIdentity::default();
    }
    if parts.len() == 1 {
        return UnderscoreIdentity {
            product: parts[0].to_string(),
            ..Default::default()
        };
    }
    let vendor = parts[0].to_string();
    if parts.len() >= 4 {
        for rev_idx in 2..parts.len() {
            let cand = parts[rev_idx];
            if looks_like_firmware_rev(cand)
                && rev_idx + 1 < parts.len()
                && parts[rev_idx + 1].len() > cand.len()
            {
                let mut firmware_date = String::new();
                let mut serial = String::new();
                let mut rest = &parts[rev_idx + 1..];
                if let Some(first) = rest.first() {
                    if looks_like_firmware_date_token(first) {
                        firmware_date = first.to_string();
                        rest = &rest[1..];
                    }
                }
                if !rest.is_empty() {
                    serial = rest.join("_");
                }
                return UnderscoreIdentity {
                    vendor,
                    product: parts[1..rev_idx].join(" "),
                    revision: cand.to_string(),
                    firmware_date,
                    serial,
                };
            }
        }
    }
    UnderscoreIdentity {
        vendor,
        product: parts[1..].join(" "),
        ..Default::default()
    }
}

/// Parse vendor / product / revision from whitespace-separated identity fields
/// after the device token (`HL-DT-ST BU40N 1.03` or multi-word product).
fn parse_inline_identity(fields: &[&str]) -> (String, String, String) {
    if fields.is_empty() {
        return (String::new(), String::new(), String::new());
    }
    let vendor = fields[0].to_string();
    let (product, revision) = match fields.len() {
        n if n >= 4 => (fields[1..n - 1].join(" "), fields[n - 1].to_string()),
        3 => (fields[1].to_string(), fields[2].to_string()),
        2 => (fields[1].to_string(), String::new()),
        _ => (String::new(), String::new()),
    };
    (vendor, product, revision)
}

/// Consume an indented continuation line under a list entry, if present.
fn take_indented_continuation<'a>(lines: &[&'a str], i: &mut usize) -> Option<&'a str> {
    if *i + 1 >= lines.len() {
        return None;
    }
    let next = lines[*i + 1];
    if !(next.starts_with(' ') || next.starts_with('\t')) {
        return None;
    }
    let ident = next.trim();
    if ident.is_empty() || split_drive_list_index_line(ident).is_some() {
        return None;
    }
    *i += 1;
    Some(ident)
}

/// Parse `sdftool -l` / `makemkvcon f -l` stdout into drives.
///
/// Supports both MakeMKV list shapes (specs/01-sdftool-spec §3.1 + real binary):
///
/// **A — device path first** (observed on current sdftool):
/// ```text
/// 00: /dev/sr0
///   HL-DT-ST_BD-RE_BU40N_1.03_SERIAL
/// 00: E:
///   HL-DT-ST_BD-RE_BU50N_GE03_SERIAL
/// 00: /IOBDServices/F49D28A7
///   HL-DT-ST_BD-RE_BU50N_GE03_SERIAL
/// ```
///
/// **B — INQUIRY first** (documented RE format):
/// ```text
/// 00:  HL-DT-ST BD-RE BU50N GE03
///   /IOBDServices/F49D28A7
/// 01:  HL-DT-ST BU40N 1.03
///   open error
/// ```
///
/// Also: one-liner `0:/dev/sr0 HL-DT-ST BU40N 1.03`. Capped at [`MAX_OPTICAL_DRIVES`].
pub fn parse_drive_list(output: &str) -> Vec<Drive> {
    let mut drives = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;
    while i < lines.len() && drives.len() < MAX_OPTICAL_DRIVES {
        let Some(rest) = split_drive_list_index_line(lines[i]) else {
            i += 1;
            continue;
        };
        if rest.is_empty() {
            i += 1;
            continue;
        }

        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.is_empty() {
            i += 1;
            continue;
        }

        let drive = if is_drive_device_token(parts[0]) {
            let device = parts[0].to_string();
            let mut drive = Drive {
                device,
                ..Default::default()
            };
            if parts.len() >= 2 {
                let (v, p, r) = parse_inline_identity(&parts[1..]);
                drive.vendor = v;
                drive.product = p;
                drive.revision = r;
            } else if let Some(ident) = take_indented_continuation(&lines, &mut i) {
                if !is_list_status_line(ident) {
                    parse_underscore_identity_full(ident).apply_to_drive(&mut drive);
                }
            }
            drive
        } else {
            let (vendor, product, revision) = parse_inline_identity(&parts);
            let mut drive = Drive {
                vendor,
                product,
                revision,
                ..Default::default()
            };
            if let Some(ident) = take_indented_continuation(&lines, &mut i) {
                if is_list_status_line(ident) {
                } else if is_drive_device_token(ident) {
                    drive.device = ident.to_string();
                } else {
                    drive.device = ident.to_string();
                    let parsed = parse_underscore_identity_full(ident);
                    if drive.vendor.is_empty() && drive.product.is_empty() {
                        parsed.apply_to_drive(&mut drive);
                    } else {
                        if !parsed.firmware_date.is_empty() {
                            drive.firmware_date = parsed.firmware_date;
                        }
                        if !parsed.serial.is_empty() {
                            drive.serial = parsed.serial;
                        }
                    }
                }
            }
            if drive.device.is_empty() && (!drive.vendor.is_empty() || !drive.product.is_empty()) {
                drive.device = drive.build_drive_id();
            }
            if drive.device.is_empty() {
                i += 1;
                continue;
            }
            drive
        };

        drives.push(drive);
        i += 1;
    }
    cap_drive_list(drives)
}

#[cfg(any(test, target_os = "macos"))]
mod mac_parsers {
    use super::{Drive, MAX_OPTICAL_DRIVES};

    /// Extract `"Key"="Value"` pairs from an IORegistry property dump block.
    fn ioreg_quoted_value(block: &str, key: &str) -> Option<String> {
        let needle = format!("\"{key}\"=");
        let idx = block.find(&needle)?;
        let rest = &block[idx + needle.len()..];
        let rest = rest.trim_start();
        if !rest.starts_with('"') {
            return None;
        }
        let end = rest[1..].find('"')?;
        Some(rest[1..1 + end].to_string())
    }

    /// Parse `ioreg -r -c <Class> -l` for Device Characteristics (Vendor/Product/Rev).
    ///
    /// MakeMKV/sdftool open these services via IOSCSITaskDeviceInterface; the true
    /// `/IOBDServices/<hash>` path is supplied by backend `-l`. OS enum uses
    /// BuildDriveId as a stable selectable id (works with `sdftool -d`).
    pub(crate) fn parse_ioreg_optical_services(output: &str, service_class: &str) -> Vec<Drive> {
        let mut drives = Vec::new();
        for chunk in output.split("Device Characteristics") {
            if chunk.len() == output.len() {
                continue;
            }
            let region = chunk.get(..500).unwrap_or(chunk);
            let vendor = ioreg_quoted_value(region, "Vendor Name").unwrap_or_default();
            let product = ioreg_quoted_value(region, "Product Name").unwrap_or_default();
            let revision = ioreg_quoted_value(region, "Product Revision Level").unwrap_or_default();
            if vendor.is_empty() && product.is_empty() {
                continue;
            }
            let mut drive = Drive {
                device: String::new(),
                vendor,
                product,
                revision,
                ..Default::default()
            };
            drive.device = format!("/{service_class}/{}", drive.build_drive_id());
            drives.push(drive);
            if drives.len() >= MAX_OPTICAL_DRIVES {
                break;
            }
        }
        drives
    }

    /// Parse `drutil list` stdout into drives.
    ///
    /// Example:
    /// ```text
    ///    Vendor   Product           Rev   Bus       SupportLevel
    /// 1  HL-DT-ST BD-RE BU50N       GE03  USB       Unsupported
    /// ```
    ///
    /// Device paths from drutil are 1-based indices (not MakeMKV `/IOBDServices/…`
    /// paths). Prefer backend `-l` when a tool is configured so flash/probe get
    /// the path sdftool actually accepts.
    pub(crate) fn parse_drutil_list(output: &str) -> Vec<Drive> {
        let mut drives = Vec::new();
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("Vendor") {
                continue;
            }
            let mut parts = trimmed.split_whitespace();
            let index = parts.next().unwrap_or("");
            if index.is_empty() || !index.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Some(vendor) = parts.next() else {
                continue;
            };
            let rest: Vec<&str> = parts.collect();
            let bus_idx = rest.iter().position(|t| {
                matches!(
                    *t,
                    "USB" | "ATAPI" | "FireWire" | "SCSI" | "SATA" | "Thunderbolt"
                )
            });
            let Some(bus_i) = bus_idx else {
                continue;
            };
            if bus_i == 0 {
                continue;
            }
            let rev_i = bus_i - 1;
            let product = rest[..rev_i].join(" ");
            let revision = rest[rev_i].to_string();
            if product.is_empty() {
                continue;
            }
            drives.push(Drive {
                device: format!("drutil:{index}"),
                vendor: vendor.to_string(),
                product,
                revision,
                ..Default::default()
            });
        }
        drives
    }

    pub(crate) fn parse_vendor_product(name: &str) -> (String, String) {
        match name.split_once('_').or_else(|| name.split_once(' ')) {
            Some((v, p)) => (v.to_string(), p.to_string()),
            None => (String::new(), name.to_string()),
        }
    }
}

#[cfg(any(test, target_os = "macos"))]
pub(crate) use mac_parsers::{
    parse_drutil_list, parse_ioreg_optical_services, parse_vendor_product,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_to_drive_match() {
        let drive = Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        };
        let dm: DriveIdentity = (&drive).into();
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BU40N");
        assert_eq!(dm.revision, "1.03");
    }

    #[test]
    fn parse_drive_list_four_fields() {
        let output = "0:/dev/sr0 HL-DT-ST BU40N 1.03\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
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

    /// Golden fixtures: real MakeMKV/sdftool multi-line shapes per OS.
    ///
    /// These always run in CI (no hardware). They exist because the old parser
    /// only accepted Linux-style one-liners and silently dropped / mis-parsed
    /// the multi-line format used on macOS, Windows, and modern Linux builds.
    #[test]
    fn parse_drive_list_golden_macos_usb_bd() {
        let output = "\
Found 1 drives(s)
00: /IOBDServices/F49D28A7
  HL-DT-ST_BD-RE_BU50N_GE03_211904231648_MODJ9TK3546

";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/IOBDServices/F49D28A7");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BD-RE BU50N");
        assert_eq!(drives[0].revision, "GE03");
    }

    #[test]
    fn parse_drive_list_golden_linux_multiline() {
        let output = "\
Found 1 drives(s)
00: /dev/sr0
  HL-DT-ST_BD-RE_BU40N_1.03_211904231648_MODJ9TK3546

";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BD-RE BU40N");
        assert_eq!(drives[0].revision, "1.03");
    }

    #[test]
    fn parse_drive_list_golden_linux_sg_device() {
        let output = "\
Found 1 drives(s)
00: /dev/sg1
  PIONEER_BD-RW_BDR-209M_1.10_ABCDE12345_EXTRA

";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sg1");
        assert_eq!(drives[0].vendor, "PIONEER");
        assert_eq!(drives[0].product, "BD-RW BDR-209M");
        assert_eq!(drives[0].revision, "1.10");
    }

    #[test]
    fn parse_drive_list_golden_windows_drive_letter_multiline() {
        let output = "\
Found 1 drives(s)
00: E:
  HL-DT-ST_BD-RE_BU50N_GE03_211904231648_MODJ9TK3546

";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "E:");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BD-RE BU50N");
        assert_eq!(drives[0].revision, "GE03");
    }

    #[test]
    fn parse_drive_list_golden_windows_extended_path() {
        let output = "\
Found 1 drives(s)
00: \\\\.\\D:
  ASUS_BW-16D1HT_3.10_SERIAL1234567_X

";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, r"\\.\D:");
        assert_eq!(drives[0].vendor, "ASUS");
        assert_eq!(drives[0].product, "BW-16D1HT");
        assert_eq!(drives[0].revision, "3.10");
    }

    #[test]
    fn parse_drive_list_golden_windows_cdrom_path() {
        let output = "00: \\\\.\\CdRom0\n  HL-DT-ST_DVD-RAM_GH24NSD1_LE00_SERIAL\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, r"\\.\CdRom0");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
    }

    #[test]
    fn parse_drive_list_multi_platform_mixed() {
        let output = "\
Found 3 drives(s)
00: /dev/sr0
  HL-DT-ST_BD-RE_BU40N_1.03_SERIALAAAAAA_X
01: E:
  HL-DT-ST_BD-RE_BU50N_GE03_SERIALBBBBBB_Y
02: /IOBDServices/F49D28A7
  HL-DT-ST_BD-RE_BU50N_GE03_SERIALCCCCCC_Z
";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 3);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[1].device, "E:");
        assert_eq!(drives[2].device, "/IOBDServices/F49D28A7");
        assert_eq!(drives[1].revision, "GE03");
        assert_eq!(drives[2].revision, "GE03");
    }

    #[test]
    fn parse_drive_list_open_error_still_lists_device() {
        let output = "\
Found 1 drives(s)
00: /dev/sr0
  open error
";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert!(drives[0].vendor.is_empty());
        assert!(drives[0].product.is_empty());
    }

    #[test]
    fn parse_drive_list_query_error_still_lists_device() {
        let output = "\
Found 1 drives(s)
00: D:
  query error
";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "D:");
    }

    #[test]
    fn parse_drive_list_macos_iodvd_path_only() {
        let output = "0: /IODVDServices/AABBCCDD\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/IODVDServices/AABBCCDD");
        assert!(drives[0].vendor.is_empty());
    }

    #[test]
    fn parse_drive_list_windows_drive_letter_inline() {
        let output = "0: E: HL-DT-ST BU40N 1.03\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "E:");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BU40N");
        assert_eq!(drives[0].revision, "1.03");
    }

    #[test]
    fn parse_drive_list_ignores_noise_and_empty() {
        let output = "Found 0 drives(s)\nDEBUG: code=00000000\n";
        assert!(parse_drive_list(output).is_empty());
        assert!(parse_drive_list("").is_empty());
    }

    #[test]
    fn parse_drive_list_two_digit_index_with_space() {
        let output = "00: /dev/sr0 PIONEER BD-RW 1.00\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[0].product, "BD-RW");
        assert_eq!(drives[0].revision, "1.00");
    }

    /// Old bug: single-char index strip turned `00: /IOBD…` into device `0:`.
    #[test]
    fn parse_drive_list_rejects_bogus_zero_colon_device() {
        let output = "\
Found 1 drives(s)
00: /IOBDServices/F49D28A7
  HL-DT-ST_BD-RE_BU50N_GE03_SERIAL_EXTRA
";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_ne!(drives[0].device, "0:");
        assert!(drives[0].device.starts_with('/'));
    }

    #[test]
    fn is_drive_device_token_accepts_known_shapes() {
        assert!(is_drive_device_token("/dev/sr0"));
        assert!(is_drive_device_token("/dev/sg1"));
        assert!(is_drive_device_token("/dev/rdisk4"));
        assert!(is_drive_device_token("/IOBDServices/F49D28A7"));
        assert!(is_drive_device_token("/IODVDServices/ABC"));
        assert!(is_drive_device_token("D:"));
        assert!(is_drive_device_token("e:"));
        assert!(is_drive_device_token(r"\\.\D:"));
        assert!(is_drive_device_token(r"\\.\CdRom0"));
        assert!(is_drive_device_token(r"\\.\cdrom12"));
        assert!(!is_drive_device_token("0:"));
        assert!(!is_drive_device_token("HL-DT-ST"));
        assert!(!is_drive_device_token(r"\\.\NotADrive"));
        assert!(!is_drive_device_token(""));
    }

    #[test]
    fn parse_underscore_identity_bu50n() {
        let id =
            parse_underscore_identity_full("HL-DT-ST_BD-RE_BU50N_GE03_211904231648_MODJ9TK3546");
        assert_eq!(id.vendor, "HL-DT-ST");
        assert_eq!(id.product, "BD-RE BU50N");
        assert_eq!(id.revision, "GE03");
        assert_eq!(id.firmware_date, "211904231648");
        assert_eq!(id.serial, "MODJ9TK3546");
        assert_eq!(
            format_firmware_date_raw(&id.firmware_date),
            "2119-04-23 16:48"
        );
    }

    #[test]
    fn parse_underscore_identity_short() {
        let id = parse_underscore_identity_full("VENDOR_MODEL");
        assert_eq!(id.vendor, "VENDOR");
        assert_eq!(id.product, "MODEL");
        assert!(id.revision.is_empty());
        assert!(id.serial.is_empty());
    }

    #[test]
    fn build_drive_id_matches_makemkv_append_str() {
        // spaces → underscores (scsihlp.cpp AppendStr)
        let d = Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BD-RE BU50N".into(),
            revision: "GE03".into(),
            ..Default::default()
        };
        assert_eq!(d.build_drive_id(), "HL-DT-ST_BD-RE_BU50N_GE03");
    }

    #[test]
    fn resolve_selection_prefers_device_then_identity() {
        let drives = vec![
            Drive {
                device: "/dev/sr0".into(),
                vendor: "A".into(),
                product: "B".into(),
                revision: "1".into(),
                ..Default::default()
            },
            Drive {
                device: "/dev/sg2".into(),
                vendor: "HL-DT-ST".into(),
                product: "BU40N".into(),
                revision: "1.03".into(),
                ..Default::default()
            },
        ];
        let prev = Drive {
            device: "/dev/sg1".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        };
        assert_eq!(
            resolve_selection(&drives, Some(&prev), Some(0)),
            Some(1),
            "identity match should win over stale index"
        );
        assert_eq!(resolve_selection(&[], Some(&prev), Some(0)), None);
        assert_eq!(resolve_selection(&drives, None, None), Some(0));
    }

    #[test]
    fn parse_drive_list_format_b_inquiry_then_path() {
        let output = "\
Found 1 drives(s)
00:  HL-DT-ST BD-RE BU50N GE03
  /IOBDServices/F49D28A7
";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/IOBDServices/F49D28A7");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BD-RE BU50N");
        assert_eq!(drives[0].revision, "GE03");
    }

    #[test]
    fn parse_drive_list_format_b_open_error_keeps_build_id_device() {
        let output = "\
Found 1 drives(s)
00:  HL-DT-ST BU40N 1.03
  open error
";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BU40N");
        assert_eq!(drives[0].revision, "1.03");
        assert_eq!(drives[0].device, "HL-DT-ST_BU40N_1.03");
    }

    #[test]
    fn parse_drive_list_caps_at_max_optical_drives() {
        let mut out = String::from("Found 20 drives(s)\n");
        for i in 0..20 {
            out.push_str(&format!(
                "{i:02}: /dev/sr{i}\n  VEN_PROD_REV1_SERIALLONG_X\n"
            ));
        }
        let drives = parse_drive_list(&out);
        assert_eq!(drives.len(), MAX_OPTICAL_DRIVES);
    }
    #[test]
    fn parse_vendor_product_underscore() {
        let (v, p) = parse_vendor_product("HL-DT-ST_BU40N");
        assert_eq!(v, "HL-DT-ST");
        assert_eq!(p, "BU40N");
    }

    #[test]
    fn parse_vendor_product_space() {
        let (v, p) = parse_vendor_product("HL-DT-ST BU40N");
        assert_eq!(v, "HL-DT-ST");
        assert_eq!(p, "BU40N");
    }

    #[test]
    fn parse_vendor_product_no_separator() {
        let (v, p) = parse_vendor_product("BU40N");
        assert!(v.is_empty());
        assert_eq!(p, "BU40N");
    }

    #[test]
    fn parse_ioreg_device_characteristics() {
        let sample = r#"
+-o IOBDServices
  | {
  |   "Device Characteristics" = {"Product Name"="BD-RE BU50N","Vendor Name"="HL-DT-ST","Product Revision Level"="GE03"}
  | }
"#;
        let drives = parse_ioreg_optical_services(sample, "IOBDServices");
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BD-RE BU50N");
        assert_eq!(drives[0].revision, "GE03");
        assert!(drives[0].device.contains("IOBDServices"));
        assert!(drives[0].device.contains("HL-DT-ST_BD-RE_BU50N_GE03"));
    }

    #[test]
    fn parse_drutil_list_usb_bd() {
        let output = "\
   Vendor   Product           Rev   Bus       SupportLevel
1  HL-DT-ST BD-RE BU50N       GE03  USB       Unsupported
";
        let drives = parse_drutil_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "drutil:1");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BD-RE BU50N");
        assert_eq!(drives[0].revision, "GE03");
    }

    #[test]
    fn parse_drutil_list_skips_header_only() {
        let output = "   Vendor   Product           Rev   Bus       SupportLevel\n";
        assert!(parse_drutil_list(output).is_empty());
    }

    #[test]
    fn parse_drutil_list_skips_noise_and_incomplete_rows() {
        let output = "\
   Vendor   Product           Rev   Bus       SupportLevel

not-a-number  junk
2
3  ONLYVENDOR
4  V  USB
5  V  Prod  USB
6  V  ProdName  1.0  NOTABUS  Unsupported
7  HL-DT-ST  BD-RE  BU50N  GE03  USB  Unsupported
";
        let drives = parse_drutil_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "drutil:7");
        assert_eq!(drives[0].product, "BD-RE BU50N");
    }

    #[test]
    fn parse_ioreg_skips_missing_quotes_and_empty_vendor_product() {
        assert!(parse_ioreg_optical_services("no dict here", "IOBDServices").is_empty());
        let unquoted = r#"
Device Characteristics
  "Vendor Name"=HL-DT-ST
  "Product Name"=BU40N
"#;
        assert!(parse_ioreg_optical_services(unquoted, "IOBDServices").is_empty());
        let empty = r#"
Device Characteristics
  "Vendor Name"=""
  "Product Name"=""
"#;
        assert!(parse_ioreg_optical_services(empty, "IOBDServices").is_empty());
    }

    #[test]
    fn parse_ioreg_caps_at_max_optical_drives() {
        let mut sample = String::new();
        for i in 0..(MAX_OPTICAL_DRIVES + 3) {
            sample.push_str(&format!(
                r#"
Device Characteristics
  "Vendor Name"="V{i}"
  "Product Name"="P{i}"
  "Product Revision Level"="R{i}"
"#
            ));
        }
        let drives = parse_ioreg_optical_services(&sample, "IOBDServices");
        assert_eq!(drives.len(), MAX_OPTICAL_DRIVES);
    }
}
