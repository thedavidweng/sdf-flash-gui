//! sdftool `--info` interpretation: platform safety, LibreDrive status, and
//! drive identity from one probe output.

use serde::{Deserialize, Serialize};

use super::parse::DriveIdentity;
use crate::firmware_db;

/// LibreDrive status from sdftool `--info` Identification SDF strings.
///
/// MakeMKV shows e.g. "Possible, not yet enabled" — not a plain bool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibreDriveStatus {
    #[default]
    Unknown,
    /// Not supported / not reported for this drive.
    NotAvailable,
    /// Supported by SDF but not yet activated on the drive.
    PossibleNotEnabled,
    /// LibreDrive active (or Drive-Specific SDF present).
    Enabled,
}

impl LibreDriveStatus {
    /// True when LibreDrive is fully enabled on the drive.
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveSafety {
    pub mt1959: bool,
    #[serde(default)]
    pub mt1939: bool,
    pub encrypted_firmware: bool,
    pub firmware_date_prefix: Option<u32>,
    pub mtk_mode: Option<char>,
    /// LibreDrive status from Identification SDF / Drive Specific SDF lines.
    #[serde(default)]
    pub libredrive: LibreDriveStatus,
    /// SDF.bin version string (e.g. `"0x00A6"`) parsed from `--info` output.
    #[serde(default)]
    pub sdf_version: Option<String>,
}

/// Everything interpreted from one `--info` probe output.
#[derive(Debug, Clone)]
pub struct ProbeInterpretation {
    pub safety: DriveSafety,
    pub identity: DriveIdentity,
}

/// Interpret sdftool `--info` output in one pass: safety and identity.
pub fn interpret_info(device: &str, info_output: &str) -> ProbeInterpretation {
    ProbeInterpretation {
        safety: classify_drive_safety(device, info_output),
        identity: parse_identity_from_info(device, info_output),
    }
}

/// Classify drive safety from a drive-list label and SDFtool `--info` output.
pub fn classify_drive_safety(drive_label: &str, info_output: &str) -> DriveSafety {
    let mt1959 = info_output
        .lines()
        .any(|line| line.contains(":MT1959") || line.contains(" MT1959"));
    let mt1939 = !mt1959
        && info_output
            .lines()
            .any(|line| line.contains(":MT1939") || line.contains(" MT1939"));
    let mtk_mode = info_output
        .lines()
        .find(|line| line.contains("mtk:19:59"))
        .and_then(extract_mtk_mode);
    let firmware_date_prefix = firmware_db::extract_firmware_date_from_text(drive_label)
        .or_else(|| firmware_db::extract_firmware_date_from_text(info_output));
    let encrypted_firmware = matches!(
        firmware_date_prefix,
        Some(prefix) if prefix >= firmware_db::ENCRYPTED_FIRMWARE_YEAR_THRESHOLD
    ) && mtk_mode != Some('M');

    let libredrive = classify_libredrive_status(info_output);
    let sdf_version = info_output
        .lines()
        .find_map(|line| line.strip_prefix("SDF.bin version: ").map(str::trim))
        .map(|s| s.to_string());

    DriveSafety {
        mt1959,
        mt1939,
        encrypted_firmware,
        firmware_date_prefix,
        mtk_mode,
        libredrive,
        sdf_version,
    }
}

/// Parse LibreDrive status from `sdftool --info` text.
///
/// Prefer explicit Identification SDF status (`Possible, not yet enabled` /
/// `Enabled`). Fall back to Drive Specific SDF present/not present.
pub(crate) fn classify_libredrive_status(info_output: &str) -> LibreDriveStatus {
    for line in info_output.lines() {
        let t = line.trim();
        let t_lower = t.to_ascii_lowercase();

        if let Some(rest) = t.strip_prefix("8102:") {
            let r = rest.trim();
            let r_lower = r.to_ascii_lowercase();
            if r.eq_ignore_ascii_case("Enabled") {
                return LibreDriveStatus::Enabled;
            }
            if r_lower.contains("not possible") || r.eq_ignore_ascii_case("Disabled") {
                return LibreDriveStatus::NotAvailable;
            }
            if r_lower.contains("possible") {
                return LibreDriveStatus::PossibleNotEnabled;
            }
            continue;
        }

        if t_lower.contains("not possible") {
            return LibreDriveStatus::NotAvailable;
        }
        if t_lower.contains("possible") && t_lower.contains("not yet") {
            return LibreDriveStatus::PossibleNotEnabled;
        }
        if t.eq_ignore_ascii_case("Enabled") {
            return LibreDriveStatus::Enabled;
        }
    }

    let has_specific_present = info_output.lines().any(|line| {
        let l = line.trim();
        l.contains("Drive Specific SDF") && l.contains("present") && !l.contains("not present")
    });
    if has_specific_present {
        return LibreDriveStatus::Enabled;
    }
    let has_specific_absent = info_output
        .lines()
        .any(|line| line.contains("Drive Specific SDF not present"));
    let mentions_libredrive = info_output.contains("LibreDrive");
    if has_specific_absent && mentions_libredrive {
        return LibreDriveStatus::NotAvailable;
    }
    LibreDriveStatus::Unknown
}

fn extract_mtk_mode(line: &str) -> Option<char> {
    line.rsplit_once(':')
        .and_then(|(_, suffix)| suffix.trim().chars().next())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpret_info_returns_safety_and_identity() {
        let output = "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n";
        let probe = interpret_info("/dev/sr0", output);
        assert!(probe.safety.mt1959);
        assert_eq!(probe.identity.vendor, "HL-DT-ST");
        assert_eq!(probe.identity.model, "BU40N");
        assert_eq!(probe.identity.revision, "1.03");
    }

    #[test]
    fn classify_non_mt1959() {
        let safety = classify_drive_safety("D: Some_Old_Drive", "no platform info");
        assert!(!safety.mt1959);
        assert!(!safety.mt1939);
        assert!(!safety.encrypted_firmware);
        assert!(safety.mtk_mode.is_none());
        assert!(safety.firmware_date_prefix.is_none());
    }

    #[test]
    fn classify_mt1939_detected() {
        let safety = classify_drive_safety("D: Some_Old_Drive", "Drive platform: MT1939");
        assert!(!safety.mt1959);
        assert!(safety.mt1939);
    }

    #[test]
    fn classify_mt1959_takes_priority_over_mt1939() {
        let output = "Drive platform: MT1959\nAlso mentions MT1939 here";
        let safety = classify_drive_safety("D: drive", output);
        assert!(safety.mt1959);
        assert!(!safety.mt1939);
    }

    #[test]
    fn classify_mtk_mode_m_not_encrypted() {
        let output = "Drive platform: MT1959\ninternal: mtk:19:59: M\n";
        let safety = classify_drive_safety("H: HL-DT-ST_BD-RE_BU40N_1.03_212005070917", output);
        assert!(safety.mt1959);
        assert!(!safety.encrypted_firmware);
        assert_eq!(safety.mtk_mode, Some('M'));
    }

    #[test]
    fn classify_encrypted_by_date_prefix() {
        let output = "Drive platform: MT1959\ninternal: mtk:19:59: H\n";
        let safety = classify_drive_safety("BU40N_21200507", output);
        assert!(safety.mt1959);
        assert!(safety.encrypted_firmware);
        assert_eq!(safety.firmware_date_prefix, Some(2120));
        assert_eq!(safety.mtk_mode, Some('H'));
    }

    #[test]
    fn classify_not_encrypted_old_date() {
        let output = "Drive platform: MT1959\ninternal: mtk:19:59: H\n";
        let safety = classify_drive_safety("BU40N_20100101", output);
        assert!(safety.mt1959);
        assert!(!safety.encrypted_firmware);
        assert_eq!(safety.firmware_date_prefix, Some(2010));
    }

    #[test]
    fn classify_date_prefix_from_info_output() {
        let output = "Drive platform: MT1959\nsome line 21210101\ninternal: mtk:19:59: H\n";
        let safety = classify_drive_safety("BU40N", output);
        assert_eq!(safety.firmware_date_prefix, Some(2121));
        assert!(safety.encrypted_firmware);
    }

    #[test]
    fn classifies_encrypted_drive() {
        let output = "Drive platform: MT1959\ninternal: mtk:19:59: H\n";
        let safety = classify_drive_safety(
            "H: HL-DT-ST_BD-RE_BU40N_1.03_212005070917_SIK04NAG90506",
            output,
        );
        assert!(safety.mt1959);
        assert!(safety.encrypted_firmware);
    }

    #[test]
    fn classify_serial_token_does_not_mask_date() {
        let safety = classify_drive_safety("BU40N_123456789_212005070917", "");
        assert_eq!(safety.firmware_date_prefix, Some(2120));
    }

    #[test]
    fn classify_invalid_date_token_rejected() {
        let safety = classify_drive_safety("BU40N_99991399", "");
        assert!(safety.firmware_date_prefix.is_none());
        assert!(!safety.encrypted_firmware);
    }

    #[test]
    fn extract_mtk_mode_various() {
        assert_eq!(extract_mtk_mode("mtk:19:59: H"), Some('H'));
        assert_eq!(extract_mtk_mode("mtk:19:59: M"), Some('M'));
        assert_eq!(extract_mtk_mode("mtk:19:59: "), None);
        assert_eq!(extract_mtk_mode("no colon here"), None);
    }

    #[test]
    fn classify_mtk_mode_whitespace() {
        let output = "Drive platform: MT1959\ninternal: mtk:19:59:   X\n";
        let safety = classify_drive_safety("BU40N", output);
        assert_eq!(safety.mtk_mode, Some('X'));
    }

    #[test]
    fn classify_no_mtk_line() {
        let output = "Drive platform: MT1959\nno mtk line here\n";
        let safety = classify_drive_safety("BU40N", output);
        assert!(safety.mt1959);
        assert!(safety.mtk_mode.is_none());
        assert!(!safety.encrypted_firmware);
    }

    #[test]
    fn libre_drive_status_helpers() {
        assert!(LibreDriveStatus::Enabled.is_enabled());
        assert!(!LibreDriveStatus::PossibleNotEnabled.is_enabled());
        assert!(!LibreDriveStatus::NotAvailable.is_enabled());
        assert!(!LibreDriveStatus::Unknown.is_enabled());
    }

    #[test]
    fn classify_libredrive_present() {
        let output = "SDF.bin version: 0x00A6\n\nDrive Specific SDF present\n";
        let safety = classify_drive_safety("D: drive", output);
        assert_eq!(safety.libredrive, LibreDriveStatus::Enabled);
        assert_eq!(safety.sdf_version.as_deref(), Some("0x00A6"));
    }

    #[test]
    fn classify_libredrive_not_present() {
        let output = "SDF.bin version: 0x00A6\n\nDrive Specific SDF not present\n";
        let safety = classify_drive_safety("D: drive", output);
        assert_eq!(safety.libredrive, LibreDriveStatus::Unknown);
        assert_eq!(safety.sdf_version.as_deref(), Some("0x00A6"));
    }

    #[test]
    fn classify_libredrive_possible_not_enabled() {
        let output = "\
SDF.bin version: 0x00A6
Drive Specific SDF not present
Identification SDF present
8000:LibreDrive Information
8013:Status
8102:Possible, not yet enabled
8001:Drive platform
:MT1959
";
        let safety = classify_drive_safety("D: drive", output);
        assert_eq!(safety.libredrive, LibreDriveStatus::PossibleNotEnabled);
        assert!(safety.mt1959);
    }

    #[test]
    fn classify_libredrive_8102_not_possible() {
        let output = "\
8000:LibreDrive Information
8013:Status
8102:Not possible
";
        assert_eq!(
            classify_libredrive_status(output),
            LibreDriveStatus::NotAvailable
        );
    }

    #[test]
    fn classify_libredrive_8102_possible_short() {
        assert_eq!(
            classify_libredrive_status("8102:Possible\n"),
            LibreDriveStatus::PossibleNotEnabled
        );
    }

    #[test]
    fn classify_libredrive_8102_unknown_status_continues() {
        assert_eq!(
            classify_libredrive_status("8102:???\nLibreDrive Information\n"),
            LibreDriveStatus::Unknown
        );
    }

    #[test]
    fn classify_libredrive_possible_capital_enabled_word() {
        assert_eq!(
            classify_libredrive_status("Status: Possible, not yet Enabled\n"),
            LibreDriveStatus::PossibleNotEnabled
        );
    }

    #[test]
    fn classify_libredrive_possible_all_lowercase_line() {
        assert_eq!(
            classify_libredrive_status("status: possible, not yet enabled\n"),
            LibreDriveStatus::PossibleNotEnabled
        );
    }

    #[test]
    fn classify_libredrive_possible_not_yet_without_comma_phrase() {
        assert_eq!(
            classify_libredrive_status("LibreDrive is possible — not yet active\n"),
            LibreDriveStatus::PossibleNotEnabled
        );
    }

    #[test]
    fn classify_libredrive_8102_enabled() {
        let output = "8013:Status\n8102:Enabled\n";
        assert_eq!(
            classify_libredrive_status(output),
            LibreDriveStatus::Enabled
        );
    }

    #[test]
    fn classify_libredrive_8102_disabled() {
        let output = "8102:Disabled\n";
        assert_eq!(
            classify_libredrive_status(output),
            LibreDriveStatus::NotAvailable
        );
    }

    #[test]
    fn classify_libredrive_bare_enabled_line() {
        let output = "LibreDrive Information\nEnabled\n";
        assert_eq!(
            classify_libredrive_status(output),
            LibreDriveStatus::Enabled
        );
    }

    #[test]
    fn classify_libredrive_absent_phrase_line() {
        let output = "some status: Not possible for this model\n";
        assert_eq!(
            classify_libredrive_status(output),
            LibreDriveStatus::NotAvailable
        );
    }

    #[test]
    fn classify_libredrive_specific_absent_with_section() {
        let output = "\
LibreDrive Information
Drive Specific SDF not present
";
        assert_eq!(
            classify_libredrive_status(output),
            LibreDriveStatus::NotAvailable
        );
    }

    #[test]
    fn classify_libredrive_mentions_without_status() {
        let output = "LibreDrive Information\n(no status line)\n";
        assert_eq!(
            classify_libredrive_status(output),
            LibreDriveStatus::Unknown
        );
    }

    #[test]
    fn classify_libredrive_absent_from_output() {
        let safety = classify_drive_safety("D: drive", "no SDF info at all");
        assert_eq!(safety.libredrive, LibreDriveStatus::Unknown);
        assert!(safety.sdf_version.is_none());
    }

    #[test]
    fn classify_sdf_version_strips_whitespace() {
        let output = "SDF.bin version: 0x00B0  \n";
        let safety = classify_drive_safety("D: drive", output);
        assert_eq!(safety.sdf_version.as_deref(), Some("0x00B0"));
    }

    #[test]
    fn drive_safety_deserialize_without_mt1939_field() {
        let json = r#"{"mt1959":true,"encrypted_firmware":false,"firmware_date_prefix":null,"mtk_mode":null}"#;
        let safety: DriveSafety = serde_json::from_str(json).unwrap();
        assert!(safety.mt1959);
        assert!(!safety.mt1939);
        assert!(!safety.encrypted_firmware);
        assert_eq!(safety.libredrive, LibreDriveStatus::Unknown);
        assert!(safety.sdf_version.is_none());
    }

    #[test]
    fn parse_drive_identity_full_output() {
        let output = "Vendor: HL-DT-ST\nProduct: BD-RE BU40N\nRevision: 1.03\n";
        let dm = parse_identity_from_info("/dev/sr0", output);
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BD-RE BU40N");
        assert_eq!(dm.revision, "1.03");
    }

    #[test]
    fn parse_drive_identity_case_insensitive() {
        let output = "vendor: LG\nproduct: BU40N\nfirmware: 1.04\n";
        let dm = parse_identity_from_info("/dev/sr0", output);
        assert_eq!(dm.vendor, "LG");
        assert_eq!(dm.model, "BU40N");
        assert_eq!(dm.revision, "1.04");
    }

    #[test]
    fn parse_drive_identity_fallback_to_device() {
        let output = "no useful info here";
        let dm = parse_identity_from_info("HL-DT-ST_BU40N_1.03", output);
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BU40N_1.03");
    }

    #[test]
    fn parse_drive_identity_empty() {
        let dm = parse_identity_from_info("/dev/sr0", "");
        assert!(dm.vendor.is_empty());
        assert!(dm.model.is_empty());
        assert!(dm.revision.is_empty());
    }

    #[test]
    fn parse_drive_identity_model_key() {
        let output = "Model: BU40N\nRevision: 1.03\n";
        let dm = parse_identity_from_info("/dev/sr0", output);
        assert_eq!(dm.model, "BU40N");
        assert_eq!(dm.revision, "1.03");
    }

    #[test]
    fn parse_drive_identity_firmware_key() {
        let output = "Firmware: 1.04\n";
        let dm = parse_identity_from_info("/dev/sr0", output);
        assert_eq!(dm.revision, "1.04");
    }

    #[test]
    fn parse_drive_identity_fallback_single_underscore() {
        let dm = parse_identity_from_info("VENDOR_MODEL", "");
        assert_eq!(dm.vendor, "VENDOR");
        assert_eq!(dm.model, "MODEL");
    }

    #[test]
    fn parse_drive_identity_underscore_empty_vendor() {
        let dm = parse_identity_from_info("_MODEL", "");
        assert!(dm.vendor.is_empty());
        assert_eq!(dm.model, "MODEL");
    }

    #[test]
    fn parse_drive_identity_underscore_empty_model() {
        let dm = parse_identity_from_info("VENDOR_", "");
        assert_eq!(dm.vendor, "VENDOR");
        assert!(dm.model.is_empty());
    }

    #[test]
    fn parse_drive_identity_whitespace_trimmed() {
        let output = "  Vendor:   HL-DT-ST  \n  Product:   BU40N  \n";
        let dm = parse_identity_from_info("/dev/sr0", output);
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BU40N");
    }
}
