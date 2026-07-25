//! Firmware identification by binary content analysis and known-hash lookup.
//!
//! Firmware files in the wild are often renamed during distribution, so
//! filename-based heuristics are unreliable. This module extracts metadata
//! directly from the firmware binary and matches against a database of
//! known firmware hashes.

use crate::platform::DriveFormFactor;
use crate::sdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashDirection {
    Upgrade,
    Downgrade,
    Same,
}

pub fn compare_versions(current: &str, target: &str) -> FlashDirection {
    if current == target {
        return FlashDirection::Same;
    }
    let c_parts: Option<Vec<u32>> = current.split(['.', '-']).map(|p| p.parse().ok()).collect();
    let t_parts: Option<Vec<u32>> = target.split(['.', '-']).map(|p| p.parse().ok()).collect();
    if let (Some(cp), Some(tp)) = (&c_parts, &t_parts) {
        let max_len = cp.len().max(tp.len());
        for i in 0..max_len {
            let c = cp.get(i).copied().unwrap_or(0);
            let t = tp.get(i).copied().unwrap_or(0);
            match c.cmp(&t) {
                std::cmp::Ordering::Less => return FlashDirection::Upgrade,
                std::cmp::Ordering::Greater => return FlashDirection::Downgrade,
                std::cmp::Ordering::Equal => continue,
            }
        }
        return FlashDirection::Same;
    }
    if current < target {
        FlashDirection::Upgrade
    } else {
        FlashDirection::Downgrade
    }
}

#[derive(Debug, Clone)]
pub struct FirmwareSdfInfo {
    pub model: Option<String>,
}

pub fn check_firmware_sdf(firmware_data: &[u8]) -> Option<FirmwareSdfInfo> {
    let mut cursor = std::io::Cursor::new(firmware_data);
    let container = sdf::parse_sdf0(&mut cursor).ok()?;
    Some(FirmwareSdfInfo {
        model: container.metadata.model,
    })
}

/// Metadata extracted by scanning the firmware binary content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareBinaryInfo {
    /// PCB type from the boot string at offset 12288 (e.g. "JB8", "BU5", "BUP3").
    pub(crate) pcb_type: Option<String>,
    /// Drive model found embedded in the binary (e.g. "BW-16D1HT", "BU40N").
    pub model: Option<String>,
    /// Form factor inferred from the PCB type.
    pub form_factor: DriveFormFactor,
}

/// Information about a known firmware, keyed by SHA-256 hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFirmware {
    pub sha256: &'static str,
    pub model: &'static str,
    pub version: &'static str,
    pub form_factor: DriveFormFactor,
    /// Whether this firmware image itself is encrypted (date ≥ 2020).
    /// Encrypted firmware requires `rawflash enc` regardless of the
    /// drive's current firmware state.
    pub is_encrypted: bool,
}

/// Database of known firmware hashes from the "All You Need Firmware Pack"
/// (MartyMcNuts) and other verified sources.
pub static KNOWN_FIRMWARES: &[KnownFirmware] = &[
    // === Internal Desktop Drives ===
    KnownFirmware {
        sha256: "6c003aa1e75170f02746e08c8df9df5e31c7f935dde93d0174eff1be2f1fa823",
        model: "BC-12D2HT",
        version: "3.11-MK",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2119-02-27
    },
    KnownFirmware {
        sha256: "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2",
        model: "BW-16D1HT",
        version: "3.02",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2117-11-24
    },
    KnownFirmware {
        sha256: "e04ed8f38ce9e85804bc0709a9321dc43f3e468ce0c0c3bc027a1c9805556f14",
        model: "BW-16D1HT",
        version: "3.10-MK",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2119-01-04
    },
    KnownFirmware {
        sha256: "bdfd6f290ba8172d4c46d8c822013255043f4ad30deb8604608fa0397bddb648",
        model: "BH16NS55",
        version: "1.02",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2115-12-11
    },
    KnownFirmware {
        sha256: "648a16d024eea31ef4901f3ac2180cbfffeda1161d70f9096335d8a6097445bc",
        model: "WH14NS40",
        version: "1.02",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2115-12-11
    },
    KnownFirmware {
        sha256: "87790f053877e3e1bdd22969d70a7b1b6ed36624bb474bfcdee6526ac6e227c0",
        model: "WH16NS40",
        version: "1.02",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2117-03-10
    },
    KnownFirmware {
        sha256: "d133893209392b74cb0c3c1225cc4c0e8b1d927f1f671261ecc82525405e47e0",
        model: "WH16NS60",
        version: "1.00",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2117-04-25
    },
    KnownFirmware {
        sha256: "c5e351d25f647599185b117f569a98c42c1fc54f6bc07d21410677afa6372510",
        model: "WH16NS60",
        version: "1.02-MK",
        form_factor: DriveFormFactor::Desktop,
        is_encrypted: false, // date 2118-10-29
    },
    // === Slim External Drives ===
    KnownFirmware {
        sha256: "3559077e3c032451137967d4f81f3e8777f8bfbf8245e68ac4732eb5cec5f404",
        model: "BU40N",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: false, // date 2119-05-14 (Buffalo BRUHD-PU3 BN12-MK)
    },
    KnownFirmware {
        sha256: "58203b539de096786cde232ee3b0ef3fe824b20d7768a96b30f6a96bdac77bfb",
        model: "BU40N",
        version: "1.00",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: false, // date 2117-05-30 (Buffalo BRUHD-PU3 BU10)
    },
    KnownFirmware {
        sha256: "f98acee01998afb043d9cfaa0c519e7e0799824a58e565789b2c3ca735204231",
        model: "BU40N",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: false, // date 2119-02-23 (Buffalo BRUHD-PU3 BU12-MK)
    },
    KnownFirmware {
        sha256: "e04aaf44157fbbec5e3c0cbf1a9ba99c81d2aeba7d420c7ece654dc515d503ff",
        model: "BP50NB40",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: true, // date 2120-05-07
    },
    KnownFirmware {
        sha256: "04f879f0bf676ede526f7d07f20f5c949caac7e00dbb1b2620d0c1601780d29a",
        model: "BP60NB10",
        version: "1.00-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: false, // date 2117-11-21
    },
    KnownFirmware {
        sha256: "cd10fc7396a2cdc77f3b49df6fa553dabc48ac5261584fc5d62cf8a866310fce",
        model: "BP60NB10",
        version: "1.02-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: true, // date 2120-05-07
    },
    KnownFirmware {
        sha256: "1227f133eb1c5315d955bb9d9cfcaf84381772a1edfef56b39ad3439b92ca4c0",
        model: "BP60NB10",
        version: "1.02-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: true, // date 2121-07-08 (NB12 variant)
    },
    KnownFirmware {
        sha256: "221ad35b7edd402353e125841893ce651064e8fc6b90368fe84ff19a85a506f4",
        model: "BU40N",
        version: "1.00",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: false, // date 2116-12-20
    },
    KnownFirmware {
        sha256: "64900e8d69212f5d729b9dfa45ef0f317378db8c8b48083a7cb99831235b5d57",
        model: "BU40N",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: false, // date 2118-10-24
    },
    KnownFirmware {
        sha256: "592e8d22d98aea9c1162c9f010fe2df29b31f6320d539336a699a78ea3af9191",
        model: "WP50NB40",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_encrypted: true, // date 2120-05-07
    },
];

/// Extract the PCB type from the boot string at offset 12288.
/// The boot string looks like "MT1959 Boot JB8 " or "MT1959 Boot BU5 ".
fn extract_pcb_type(data: &[u8]) -> Option<String> {
    const BOOT_OFFSET: usize = 12288;
    const BOOT_LEN: usize = 20;
    if data.len() < BOOT_OFFSET + BOOT_LEN {
        return None;
    }
    let slice = &data[BOOT_OFFSET..BOOT_OFFSET + BOOT_LEN];
    let text = std::str::from_utf8(slice).ok()?;
    if !text.starts_with("MT1959 Boot ") {
        return None;
    }
    let pcb = text["MT1959 Boot ".len()..].trim_end_matches([' ', '\0']);
    if pcb.is_empty() {
        return None;
    }
    Some(pcb.to_string())
}

/// Infer form factor from PCB type.
/// JB* = desktop (JB8/JB9 PCB family), BU*/BP* = slim.
fn pcb_to_form_factor(pcb: &str) -> DriveFormFactor {
    if pcb.starts_with("JB") {
        DriveFormFactor::Desktop
    } else if pcb.starts_with("BU") || pcb.starts_with("BP") {
        DriveFormFactor::Slim
    } else {
        DriveFormFactor::Unknown
    }
}

/// Search for a known drive model name in the firmware binary.
/// Uses byte-level matching so it works on non-UTF-8 binary data.
fn extract_model(data: &[u8]) -> Option<String> {
    let search_region = &data[..data.len().min(256 * 1024)];
    crate::platform::known_models()
        .find(|m| search_region.windows(m.len()).any(|w| w == m.as_bytes()))
        .map(str::to_string)
}

/// Analyze a firmware binary to extract metadata without relying on filename.
pub fn analyze_firmware_binary(data: &[u8]) -> FirmwareBinaryInfo {
    let pcb_type = extract_pcb_type(data);
    let form_factor = pcb_type
        .as_deref()
        .map(pcb_to_form_factor)
        .unwrap_or(DriveFormFactor::Unknown);
    let model = extract_model(data);
    FirmwareBinaryInfo {
        pcb_type,
        model,
        form_factor,
    }
}

/// Year threshold for encrypted firmware (matches MakeMKV forum guide:
/// "Firmware with a date in 2020 or later is encrypted").
pub const ENCRYPTED_FIRMWARE_YEAR_THRESHOLD: u32 = 2120;

/// Extract a firmware date stamp from the binary content.
///
/// MakeMKV embeds a 12-digit ASCII calendar stamp (`YYMMDDHHMMSS`, e.g.
/// `212005070917` = 2020-05-07 09:17) inside the firmware image. This scans
/// the binary for the first plausible date sequence and returns the 4-digit
/// year prefix (e.g. `2120`).
///
/// The offset varies between firmware variants (~1.36–1.61 MB), so this
/// scans the entire binary rather than reading a fixed offset. The scan is
/// cheap (single pass, no allocation) and only matches 10–12 digit ASCII
/// sequences that parse as valid dates.
pub fn extract_firmware_date_from_binary(data: &[u8]) -> Option<u32> {
    let mut i = 0;
    while i < data.len() {
        if !data[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < data.len() && data[i].is_ascii_digit() {
            i += 1;
        }
        let run_len = i - start;
        if !(10..=12).contains(&run_len) {
            continue;
        }
        let s = std::str::from_utf8(&data[start..i]).expect("ascii digits are valid utf-8");
        if let Some(year_prefix) = parse_date_prefix(s) {
            return Some(year_prefix);
        }
    }
    None
}

/// Validate a 10–12 digit string as a firmware date and return the 4-digit
/// year prefix. The caller guarantees the input is 10–12 ASCII digits.
fn parse_date_prefix(s: &str) -> Option<u32> {
    let year: u32 = s[0..4].parse().ok()?;
    let month: u32 = s[4..6].parse().ok()?;
    let day: u32 = s[6..8].parse().ok()?;
    if !(2000..=2199).contains(&year) {
        return None;
    }
    if !(1..=12).contains(&month) {
        return None;
    }
    if !(1..=31).contains(&day) {
        return None;
    }
    Some(year)
}

/// Determine whether a firmware binary is encrypted (date ≥ 2020).
///
/// Priority:
/// 1. Known firmware hash lookup (`known.is_encrypted`) — 100% accurate.
/// 2. Binary date extraction — scans the firmware content for an embedded
///    date stamp and checks if the year ≥ 2120.
/// 3. Returns `None` if neither source yields a result (caller should fall
///    back to the drive's current firmware state).
pub fn resolve_firmware_encrypted(id: &FirmwareIdentification, data: &[u8]) -> Option<bool> {
    if let Some(known) = id.known {
        return Some(known.is_encrypted);
    }
    let date_prefix = extract_firmware_date_from_binary(data)?;
    Some(date_prefix >= ENCRYPTED_FIRMWARE_YEAR_THRESHOLD)
}

/// Look up a firmware by its SHA-256 hash in the known database.
pub fn lookup_known_firmware(sha256: &str) -> Option<&'static KnownFirmware> {
    KNOWN_FIRMWARES.iter().find(|fw| fw.sha256 == sha256)
}

/// Combined result of firmware identification.
#[derive(Debug, Clone)]
pub struct FirmwareIdentification {
    /// SHA-256 hash of the firmware binary.
    pub sha256: String,
    /// Known firmware entry if hash matched.
    pub known: Option<&'static KnownFirmware>,
    /// Metadata extracted from binary content analysis.
    pub binary_info: FirmwareBinaryInfo,
}

/// Identify a firmware binary by hash lookup + binary content analysis.
pub fn identify_firmware(data: &[u8]) -> FirmwareIdentification {
    let sha256 = sha256_hex(data);
    let known = lookup_known_firmware(&sha256);
    let binary_info = analyze_firmware_binary(data);
    FirmwareIdentification {
        sha256,
        known,
        binary_info,
    }
}

/// Best-effort form factor determination.
/// Known firmware hash takes priority, then binary PCB type, then SDF0 metadata.
pub fn resolve_form_factor(id: &FirmwareIdentification) -> DriveFormFactor {
    if let Some(known) = id.known {
        return known.form_factor;
    }
    id.binary_info.form_factor
}

/// Best-effort form factor determination with SDF0 metadata fallback.
/// Priority: known hash > binary PCB type > SDF0 model > Unknown.
pub fn resolve_form_factor_with_sdf(
    id: &FirmwareIdentification,
    sdf_info: Option<&FirmwareSdfInfo>,
) -> DriveFormFactor {
    let ff = resolve_form_factor(id);
    if ff != DriveFormFactor::Unknown {
        return ff;
    }
    if let Some(model) = sdf_info.and_then(|s| s.model.as_deref()) {
        return crate::platform::classify_drive(model);
    }
    DriveFormFactor::Unknown
}

/// Best-effort model determination.
/// Known firmware hash takes priority, then binary content, then SDF0 metadata.
pub fn resolve_model(id: &FirmwareIdentification) -> Option<String> {
    id.known
        .map(|k| k.model.to_string())
        .or_else(|| id.binary_info.model.clone())
}

/// Best-effort model determination with SDF0 metadata fallback.
pub fn resolve_model_with_sdf(
    id: &FirmwareIdentification,
    sdf_info: Option<&FirmwareSdfInfo>,
) -> Option<String> {
    resolve_model(id).or_else(|| sdf_info.and_then(|sdf| sdf.model.clone()))
}

/// Full result of firmware identification: hash lookup, binary analysis,
/// SDF0 metadata, and the resolved form factor / model / encryption status.
///
/// Produced by [`identify`], the single deep entry point that runs the whole
/// identification cascade (known hash → binary PCB → SDF0 metadata → date).
/// Callers store this struct and read its fields instead of orchestrating
/// the four `resolve_*` functions themselves.
#[derive(Debug, Clone)]
pub struct ResolvedFirmware {
    pub identification: FirmwareIdentification,
    pub sdf_info: Option<FirmwareSdfInfo>,
    pub form_factor: DriveFormFactor,
    pub model: Option<String>,
    pub encrypted: Option<bool>,
}

/// Identify a firmware binary in one deep call.
///
/// Runs the full cascade: SHA-256 hash lookup, binary content analysis
/// (PCB type + embedded model), SDF0 metadata parse, form-factor resolution
/// (known hash → binary PCB → SDF0 model), model resolution, and encryption
/// detection (known `is_encrypted` → binary date stamp ≥ 2120).
///
/// This is the single source of truth for firmware properties per ADR 0001.
/// The GUI and CLI should call this instead of threading the four `resolve_*`
/// functions themselves.
pub fn identify(data: &[u8]) -> ResolvedFirmware {
    let sdf_info = check_firmware_sdf(data);
    let identification = identify_firmware(data);
    let form_factor = resolve_form_factor_with_sdf(&identification, sdf_info.as_ref());
    let model = resolve_model_with_sdf(&identification, sdf_info.as_ref());
    let encrypted = resolve_firmware_encrypted(&identification, data);
    ResolvedFirmware {
        identification,
        sdf_info,
        form_factor,
        model,
        encrypted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_known_input() {
        let hash = sha256_hex(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hex_hello() {
        let hash = sha256_hex(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn compare_versions_upgrade() {
        assert_eq!(compare_versions("1.03", "1.04"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("1.0", "2.0"), FlashDirection::Upgrade);
    }

    #[test]
    fn compare_versions_downgrade() {
        assert_eq!(compare_versions("1.04", "1.03"), FlashDirection::Downgrade);
        assert_eq!(compare_versions("2.0", "1.0"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_same() {
        assert_eq!(compare_versions("1.03", "1.03"), FlashDirection::Same);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), FlashDirection::Same);
    }

    #[test]
    fn compare_versions_same_numeric_different_text() {
        assert_eq!(compare_versions("1.0", "1.00"), FlashDirection::Same);
    }

    #[test]
    fn compare_versions_different_lengths() {
        assert_eq!(compare_versions("1.0", "1.0.1"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("1.0.1", "1.0"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_non_numeric_fallback() {
        assert_eq!(compare_versions("abc", "def"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("def", "abc"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_mixed_numeric_non_numeric() {
        assert_eq!(compare_versions("1.0", "abc"), FlashDirection::Upgrade);
    }

    #[test]
    fn check_firmware_sdf_non_sdf_data() {
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(&[0x85, 0x4a, 0xc0, 0x75]);
        assert!(check_firmware_sdf(&data).is_none());
    }

    #[test]
    fn check_firmware_sdf_valid_sdf0() {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        let metadata = b"Vendor\0TestVendor\0Model\0TestModel\0";
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_be_bytes());
        data.extend_from_slice(metadata);

        let info = check_firmware_sdf(&data).unwrap();
        assert_eq!(info.model.as_deref(), Some("TestModel"));
    }

    #[test]
    fn check_firmware_sdf_padded_metadata_region() {
        let metadata = b"Vendor\0TestVendor\0Model\0TestModel\0";
        let payload_offset = 256u32;
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&payload_offset.to_be_bytes());
        data.extend_from_slice(metadata);
        data.resize(payload_offset as usize, 0xAA);
        data.extend(vec![0u8; 64]);

        let info = check_firmware_sdf(&data).unwrap();
        assert_eq!(info.model.as_deref(), Some("TestModel"));
    }

    #[test]
    fn known_firmware_database_has_entries() {
        assert!(!KNOWN_FIRMWARES.is_empty());
        assert!(KNOWN_FIRMWARES.len() >= 18);
    }

    #[test]
    fn known_firmware_hashes_are_unique() {
        let mut hashes: Vec<_> = KNOWN_FIRMWARES.iter().map(|f| f.sha256).collect();
        hashes.sort();
        let len_before = hashes.len();
        hashes.dedup();
        assert_eq!(hashes.len(), len_before, "duplicate SHA-256 in database");
    }

    #[test]
    fn lookup_known_firmware_finds_match() {
        let fw = lookup_known_firmware(
            "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2",
        );
        assert!(fw.is_some());
        let fw = fw.unwrap();
        assert_eq!(fw.model, "BW-16D1HT");
        assert_eq!(fw.version, "3.02");
        assert_eq!(fw.form_factor, DriveFormFactor::Desktop);
    }

    #[test]
    fn lookup_known_firmware_unknown_hash() {
        let fw = lookup_known_firmware(
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(fw.is_none());
    }

    #[test]
    fn extract_pcb_type_desktop() {
        let mut data = vec![0u8; 12310];
        let boot = b"MT1959 Boot JB8 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let pcb = extract_pcb_type(&data);
        assert_eq!(pcb.as_deref(), Some("JB8"));
    }

    #[test]
    fn extract_pcb_type_slim() {
        let mut data = vec![0u8; 12310];
        let boot = b"MT1959 Boot BU5 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let pcb = extract_pcb_type(&data);
        assert_eq!(pcb.as_deref(), Some("BU5"));
    }

    #[test]
    fn extract_pcb_type_slim_bup3() {
        let mut data = vec![0u8; 12310];
        let boot = b"MT1959 Boot BUP3";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let pcb = extract_pcb_type(&data);
        assert_eq!(pcb.as_deref(), Some("BUP3"));
    }

    #[test]
    fn extract_pcb_type_data_too_short() {
        let data = vec![0u8; 100];
        assert!(extract_pcb_type(&data).is_none());
    }

    #[test]
    fn extract_pcb_type_boundary_length_12307() {
        let data = vec![0u8; 12307];
        assert!(extract_pcb_type(&data).is_none());
    }

    #[test]
    fn extract_pcb_type_boundary_length_12308() {
        let mut data = vec![0u8; 12308];
        let boot = b"MT1959 Boot JB8 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        assert_eq!(extract_pcb_type(&data).as_deref(), Some("JB8"));
    }

    #[test]
    fn extract_pcb_type_no_boot_string() {
        let data = vec![0u8; 13000];
        assert!(extract_pcb_type(&data).is_none());
    }

    #[test]
    fn pcb_to_form_factor_jb_is_desktop() {
        assert_eq!(pcb_to_form_factor("JB8"), DriveFormFactor::Desktop);
        assert_eq!(pcb_to_form_factor("JBC6"), DriveFormFactor::Desktop);
    }

    #[test]
    fn pcb_to_form_factor_bu_bp_is_slim() {
        assert_eq!(pcb_to_form_factor("BU5"), DriveFormFactor::Slim);
        assert_eq!(pcb_to_form_factor("BUP3"), DriveFormFactor::Slim);
        assert_eq!(pcb_to_form_factor("BUP5"), DriveFormFactor::Slim);
        assert_eq!(pcb_to_form_factor("BP52"), DriveFormFactor::Slim);
    }

    #[test]
    fn pcb_to_form_factor_unknown() {
        assert_eq!(pcb_to_form_factor("XX1"), DriveFormFactor::Unknown);
    }

    #[test]
    fn extract_model_finds_bw16d1ht() {
        let mut data = vec![0u8; 40000];
        let model = b"ASUS    BW-16D1HT       BOOT";
        data[37600..37600 + model.len()].copy_from_slice(model);
        let found = extract_model(&data);
        assert_eq!(found.as_deref(), Some("BW-16D1HT"));
    }

    #[test]
    fn extract_model_finds_bu40n() {
        let mut data = vec![0u8; 40000];
        let model = b"HL-DT-STBD-RE BU40N     BOOT";
        data[37900..37900 + model.len()].copy_from_slice(model);
        let found = extract_model(&data);
        assert_eq!(found.as_deref(), Some("BU40N"));
    }

    #[test]
    fn extract_model_not_found() {
        let data = vec![0u8; 40000];
        assert!(extract_model(&data).is_none());
    }

    #[test]
    fn extract_model_works_on_non_utf8_binary() {
        let mut data = vec![0xFFu8; 40000];
        let model = b"BW-16D1HT";
        data[37600..37600 + model.len()].copy_from_slice(model);
        let found = extract_model(&data);
        assert_eq!(found.as_deref(), Some("BW-16D1HT"));
    }

    #[test]
    fn extract_model_works_with_mixed_binary_data() {
        let mut data = vec![0u8; 40000];
        for i in 0..30000 {
            data[i] = 0x80 + (i % 100) as u8;
        }
        let model = b"BU40N";
        data[37900..37900 + model.len()].copy_from_slice(model);
        let found = extract_model(&data);
        assert_eq!(found.as_deref(), Some("BU40N"));
    }

    #[test]
    fn analyze_firmware_binary_desktop() {
        let mut data = vec![0u8; 40000];
        let boot = b"MT1959 Boot JB8 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let model = b"BW-16D1HT";
        data[37600..37600 + model.len()].copy_from_slice(model);
        let info = analyze_firmware_binary(&data);
        assert_eq!(info.pcb_type.as_deref(), Some("JB8"));
        assert_eq!(info.model.as_deref(), Some("BW-16D1HT"));
        assert_eq!(info.form_factor, DriveFormFactor::Desktop);
    }

    #[test]
    fn analyze_firmware_binary_slim() {
        let mut data = vec![0u8; 40000];
        let boot = b"MT1959 Boot BU5 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let model = b"BU40N";
        data[37900..37900 + model.len()].copy_from_slice(model);
        let info = analyze_firmware_binary(&data);
        assert_eq!(info.pcb_type.as_deref(), Some("BU5"));
        assert_eq!(info.model.as_deref(), Some("BU40N"));
        assert_eq!(info.form_factor, DriveFormFactor::Slim);
    }

    #[test]
    fn analyze_firmware_binary_empty_data() {
        let info = analyze_firmware_binary(&[]);
        assert!(info.pcb_type.is_none());
        assert!(info.model.is_none());
        assert_eq!(info.form_factor, DriveFormFactor::Unknown);
    }

    #[test]
    fn resolve_form_factor_known_takes_priority() {
        let id = FirmwareIdentification {
            sha256: "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2".to_string(),
            known: lookup_known_firmware(
                "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2",
            ),
            binary_info: FirmwareBinaryInfo {
                pcb_type: Some("BU5".to_string()),
                model: Some("BU40N".to_string()),
                form_factor: DriveFormFactor::Slim,
            },
        };
        assert_eq!(resolve_form_factor(&id), DriveFormFactor::Desktop);
    }

    #[test]
    fn resolve_form_factor_falls_back_to_binary() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: Some("JB8".to_string()),
                model: None,
                form_factor: DriveFormFactor::Desktop,
            },
        };
        assert_eq!(resolve_form_factor(&id), DriveFormFactor::Desktop);
    }

    #[test]
    fn resolve_form_factor_unknown_when_no_info() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert_eq!(resolve_form_factor(&id), DriveFormFactor::Unknown);
    }

    #[test]
    fn resolve_model_known_takes_priority() {
        let id = FirmwareIdentification {
            sha256: "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2".to_string(),
            known: lookup_known_firmware(
                "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2",
            ),
            binary_info: FirmwareBinaryInfo {
                pcb_type: Some("JB8".to_string()),
                model: Some("WRONG".to_string()),
                form_factor: DriveFormFactor::Desktop,
            },
        };
        assert_eq!(resolve_model(&id).as_deref(), Some("BW-16D1HT"));
    }

    #[test]
    fn identify_firmware_unknown_binary() {
        let data = vec![0u8; 100];
        let id = identify_firmware(&data);
        assert!(id.known.is_none());
        assert_eq!(id.binary_info.form_factor, DriveFormFactor::Unknown);
        assert!(!id.sha256.is_empty());
    }

    #[test]
    fn identify_unknown_binary_returns_unknown_form_factor_and_no_encryption() {
        let data = vec![0u8; 100];
        let resolved = identify(&data);
        assert!(resolved.identification.known.is_none());
        assert_eq!(resolved.form_factor, DriveFormFactor::Unknown);
        assert!(resolved.model.is_none());
        assert!(resolved.encrypted.is_none());
        assert!(resolved.sdf_info.is_none());
    }

    #[test]
    fn identify_desktop_firmware_resolves_form_factor_and_model_from_binary() {
        let mut data = vec![0u8; 40000];
        let boot = b"MT1959 Boot JB8 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let model = b"BW-16D1HT";
        data[37600..37600 + model.len()].copy_from_slice(model);
        let resolved = identify(&data);
        assert_eq!(resolved.form_factor, DriveFormFactor::Desktop);
        assert_eq!(resolved.model.as_deref(), Some("BW-16D1HT"));
    }

    #[test]
    fn identify_slim_firmware_resolves_form_factor_and_model_from_binary() {
        let mut data = vec![0u8; 40000];
        let boot = b"MT1959 Boot BU5 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let model = b"BU40N";
        data[37900..37900 + model.len()].copy_from_slice(model);
        let resolved = identify(&data);
        assert_eq!(resolved.form_factor, DriveFormFactor::Slim);
        assert_eq!(resolved.model.as_deref(), Some("BU40N"));
    }

    #[test]
    fn identify_encrypted_firmware_from_binary_date() {
        let mut data = vec![0u8; 100_000];
        let date = b"212005070917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        let resolved = identify(&data);
        assert_eq!(resolved.encrypted, Some(true));
    }

    #[test]
    fn identify_non_encrypted_firmware_from_binary_date() {
        let mut data = vec![0u8; 100_000];
        let date = b"211810291936";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        let resolved = identify(&data);
        assert_eq!(resolved.encrypted, Some(false));
    }

    #[test]
    fn identify_uses_sdf_metadata_when_binary_pcb_absent() {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        let metadata = b"Vendor\0HL-DT-ST\0Model\0BU40N\0";
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_be_bytes());
        data.extend_from_slice(metadata);
        data.resize(payload_offset as usize, 0xAA);
        data.extend(vec![0u8; 64]);
        let resolved = identify(&data);
        assert_eq!(resolved.form_factor, DriveFormFactor::Slim);
        assert_eq!(resolved.model.as_deref(), Some("BU40N"));
        assert!(resolved.sdf_info.is_some());
    }

    #[test]
    fn extract_pcb_type_empty_after_trim() {
        let mut data = vec![0u8; 12310];
        let boot = b"MT1959 Boot     ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        assert!(extract_pcb_type(&data).is_none());
    }

    #[test]
    fn resolve_form_factor_with_sdf_fallback() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        let sdf = FirmwareSdfInfo {
            model: Some("BU40N".to_string()),
        };
        assert_eq!(
            resolve_form_factor_with_sdf(&id, Some(&sdf)),
            DriveFormFactor::Slim
        );
    }

    #[test]
    fn resolve_form_factor_with_sdf_no_model() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        let sdf = FirmwareSdfInfo { model: None };
        assert_eq!(
            resolve_form_factor_with_sdf(&id, Some(&sdf)),
            DriveFormFactor::Unknown
        );
    }

    #[test]
    fn resolve_form_factor_with_sdf_none_info() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert_eq!(
            resolve_form_factor_with_sdf(&id, None),
            DriveFormFactor::Unknown
        );
    }

    #[test]
    fn resolve_model_binary_fallback() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: Some("JB8".to_string()),
                model: Some("BW-16D1HT".to_string()),
                form_factor: DriveFormFactor::Desktop,
            },
        };
        assert_eq!(resolve_model(&id).as_deref(), Some("BW-16D1HT"));
    }

    #[test]
    fn resolve_model_no_info() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert!(resolve_model(&id).is_none());
    }

    #[test]
    fn resolve_model_with_sdf_known_takes_priority() {
        let id = FirmwareIdentification {
            sha256: "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2".to_string(),
            known: lookup_known_firmware(
                "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2",
            ),
            binary_info: FirmwareBinaryInfo {
                pcb_type: Some("JB8".to_string()),
                model: Some("WRONG".to_string()),
                form_factor: DriveFormFactor::Desktop,
            },
        };
        let sdf = FirmwareSdfInfo {
            model: Some("ALSO_WRONG".to_string()),
        };
        assert_eq!(
            resolve_model_with_sdf(&id, Some(&sdf)).as_deref(),
            Some("BW-16D1HT")
        );
    }

    #[test]
    fn resolve_model_with_sdf_binary_fallback() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: Some("JB8".to_string()),
                model: Some("BW-16D1HT".to_string()),
                form_factor: DriveFormFactor::Desktop,
            },
        };
        let sdf = FirmwareSdfInfo {
            model: Some("SHOULD_NOT_USE".to_string()),
        };
        assert_eq!(
            resolve_model_with_sdf(&id, Some(&sdf)).as_deref(),
            Some("BW-16D1HT")
        );
    }

    #[test]
    fn resolve_model_with_sdf_sdf_fallback() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        let sdf = FirmwareSdfInfo {
            model: Some("BU40N".to_string()),
        };
        assert_eq!(
            resolve_model_with_sdf(&id, Some(&sdf)).as_deref(),
            Some("BU40N")
        );
    }

    #[test]
    fn resolve_model_with_sdf_no_info() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert!(resolve_model_with_sdf(&id, None).is_none());
    }

    #[test]
    fn resolve_model_with_sdf_sdf_no_model() {
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        let sdf = FirmwareSdfInfo { model: None };
        assert!(resolve_model_with_sdf(&id, Some(&sdf)).is_none());
    }

    #[test]
    fn extract_firmware_date_finds_encrypted_date() {
        let mut data = vec![0u8; 1_400_000];
        let date = b"212005070917";
        data[1_370_000..1_370_000 + date.len()].copy_from_slice(date);
        let prefix = extract_firmware_date_from_binary(&data);
        assert_eq!(prefix, Some(2120));
    }

    #[test]
    fn extract_firmware_date_finds_non_encrypted_date() {
        let mut data = vec![0u8; 1_400_000];
        let date = b"211810291936";
        data[1_370_000..1_370_000 + date.len()].copy_from_slice(date);
        let prefix = extract_firmware_date_from_binary(&data);
        assert_eq!(prefix, Some(2118));
    }

    #[test]
    fn extract_firmware_date_finds_10_digit_stamp() {
        let mut data = vec![0u8; 100_000];
        let date = b"2120050709";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        let prefix = extract_firmware_date_from_binary(&data);
        assert_eq!(prefix, Some(2120));
    }

    #[test]
    fn extract_firmware_date_none_when_no_date() {
        let data = vec![0u8; 100_000];
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_none_for_short_digit_runs() {
        let mut data = vec![0u8; 1000];
        let digits = b"212005070";
        data[100..100 + digits.len()].copy_from_slice(digits);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_rejects_invalid_month() {
        let mut data = vec![0u8; 100_000];
        let date = b"212013070917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_rejects_invalid_day() {
        let mut data = vec![0u8; 100_000];
        let date = b"212005320917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_rejects_year_out_of_range() {
        let mut data = vec![0u8; 100_000];
        let date = b"199005070917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_skips_short_digit_runs_finds_real_date() {
        let mut data = vec![0u8; 100_000];
        data[100..105].copy_from_slice(b"12345");
        let date = b"211810291936";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        let prefix = extract_firmware_date_from_binary(&data);
        assert_eq!(prefix, Some(2118));
    }

    #[test]
    fn extract_firmware_date_empty_data() {
        assert!(extract_firmware_date_from_binary(&[]).is_none());
    }

    #[test]
    fn resolve_firmware_encrypted_known_encrypted() {
        let hash = "e04aaf44157fbbec5e3c0cbf1a9ba99c81d2aeba7d420c7ece654dc515d503ff";
        let id = FirmwareIdentification {
            sha256: hash.to_string(),
            known: lookup_known_firmware(hash),
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert_eq!(resolve_firmware_encrypted(&id, &[]), Some(true));
    }

    #[test]
    fn resolve_firmware_encrypted_known_non_encrypted() {
        let hash = "c5e351d25f647599185b117f569a98c42c1fc54f6bc07d21410677afa6372510";
        let id = FirmwareIdentification {
            sha256: hash.to_string(),
            known: lookup_known_firmware(hash),
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert_eq!(resolve_firmware_encrypted(&id, &[]), Some(false));
    }

    #[test]
    fn resolve_firmware_encrypted_unknown_falls_back_to_binary_encrypted() {
        let mut data = vec![0u8; 100_000];
        let date = b"212005070917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert_eq!(resolve_firmware_encrypted(&id, &data), Some(true));
    }

    #[test]
    fn resolve_firmware_encrypted_unknown_falls_back_to_binary_non_encrypted() {
        let mut data = vec![0u8; 100_000];
        let date = b"211810291936";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert_eq!(resolve_firmware_encrypted(&id, &data), Some(false));
    }

    #[test]
    fn resolve_firmware_encrypted_unknown_no_date_returns_none() {
        let data = vec![0u8; 100_000];
        let id = FirmwareIdentification {
            sha256: "unknown".to_string(),
            known: None,
            binary_info: FirmwareBinaryInfo {
                pcb_type: None,
                model: None,
                form_factor: DriveFormFactor::Unknown,
            },
        };
        assert!(resolve_firmware_encrypted(&id, &data).is_none());
    }

    #[test]
    fn known_firmware_encrypted_flags_are_set() {
        let encrypted: Vec<_> = KNOWN_FIRMWARES
            .iter()
            .filter(|f| f.is_encrypted)
            .map(|f| (f.model, f.version))
            .collect();
        assert_eq!(encrypted.len(), 4);
        assert!(encrypted
            .iter()
            .all(|(m, _)| m.starts_with("BP") || m.starts_with("WP")));
    }

    #[test]
    fn known_firmware_non_encrypted_flags_are_false() {
        let non_encrypted = KNOWN_FIRMWARES.iter().filter(|f| !f.is_encrypted).count();
        assert_eq!(non_encrypted, 14);
    }
}
