//! Firmware identification by binary content analysis and known-hash lookup.
//!
//! Firmware files in the wild are often renamed during distribution, so
//! filename-based heuristics are unreliable. This module extracts metadata
//! directly from the firmware binary and matches against a database of
//! known firmware hashes.

use crate::platform::DriveFormFactor;

/// Metadata extracted by scanning the firmware binary content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareBinaryInfo {
    /// PCB type from the boot string at offset 12288 (e.g. "JB8", "BU5", "BUP3").
    pub pcb_type: Option<String>,
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
    pub is_mk: bool,
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
        is_mk: true,
        is_encrypted: false, // date 2119-02-27
    },
    KnownFirmware {
        sha256: "83ea24bb07b8a7a451bba1856d1db18b7f54b1823b9d148e213f25daf2e0a1d2",
        model: "BW-16D1HT",
        version: "3.02",
        form_factor: DriveFormFactor::Desktop,
        is_mk: false,
        is_encrypted: false, // date 2117-11-24
    },
    KnownFirmware {
        sha256: "e04ed8f38ce9e85804bc0709a9321dc43f3e468ce0c0c3bc027a1c9805556f14",
        model: "BW-16D1HT",
        version: "3.10-MK",
        form_factor: DriveFormFactor::Desktop,
        is_mk: true,
        is_encrypted: false, // date 2119-01-04
    },
    KnownFirmware {
        sha256: "bdfd6f290ba8172d4c46d8c822013255043f4ad30deb8604608fa0397bddb648",
        model: "BH16NS55",
        version: "1.02",
        form_factor: DriveFormFactor::Desktop,
        is_mk: false,
        is_encrypted: false, // date 2115-12-11
    },
    KnownFirmware {
        sha256: "648a16d024eea31ef4901f3ac2180cbfffeda1161d70f9096335d8a6097445bc",
        model: "WH14NS40",
        version: "1.02",
        form_factor: DriveFormFactor::Desktop,
        is_mk: false,
        is_encrypted: false, // date 2115-12-11
    },
    KnownFirmware {
        sha256: "87790f053877e3e1bdd22969d70a7b1b6ed36624bb474bfcdee6526ac6e227c0",
        model: "WH16NS40",
        version: "1.02",
        form_factor: DriveFormFactor::Desktop,
        is_mk: false,
        is_encrypted: false, // date 2117-03-10
    },
    KnownFirmware {
        sha256: "d133893209392b74cb0c3c1225cc4c0e8b1d927f1f671261ecc82525405e47e0",
        model: "WH16NS60",
        version: "1.00",
        form_factor: DriveFormFactor::Desktop,
        is_mk: false,
        is_encrypted: false, // date 2117-04-25
    },
    KnownFirmware {
        sha256: "c5e351d25f647599185b117f569a98c42c1fc54f6bc07d21410677afa6372510",
        model: "WH16NS60",
        version: "1.02-MK",
        form_factor: DriveFormFactor::Desktop,
        is_mk: true,
        is_encrypted: false, // date 2118-10-29
    },
    // === Slim External Drives ===
    KnownFirmware {
        sha256: "3559077e3c032451137967d4f81f3e8777f8bfbf8245e68ac4732eb5cec5f404",
        model: "BU40N",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: false, // date 2119-05-14 (Buffalo BRUHD-PU3 BN12-MK)
    },
    KnownFirmware {
        sha256: "58203b539de096786cde232ee3b0ef3fe824b20d7768a96b30f6a96bdac77bfb",
        model: "BU40N",
        version: "1.00",
        form_factor: DriveFormFactor::Slim,
        is_mk: false,
        is_encrypted: false, // date 2117-05-30 (Buffalo BRUHD-PU3 BU10)
    },
    KnownFirmware {
        sha256: "f98acee01998afb043d9cfaa0c519e7e0799824a58e565789b2c3ca735204231",
        model: "BU40N",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: false, // date 2119-02-23 (Buffalo BRUHD-PU3 BU12-MK)
    },
    KnownFirmware {
        sha256: "e04aaf44157fbbec5e3c0cbf1a9ba99c81d2aeba7d420c7ece654dc515d503ff",
        model: "BP50NB40",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: true, // date 2120-05-07
    },
    KnownFirmware {
        sha256: "04f879f0bf676ede526f7d07f20f5c949caac7e00dbb1b2620d0c1601780d29a",
        model: "BP60NB10",
        version: "1.00-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: false, // date 2117-11-21
    },
    KnownFirmware {
        sha256: "cd10fc7396a2cdc77f3b49df6fa553dabc48ac5261584fc5d62cf8a866310fce",
        model: "BP60NB10",
        version: "1.02-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: true, // date 2120-05-07
    },
    KnownFirmware {
        sha256: "1227f133eb1c5315d955bb9d9cfcaf84381772a1edfef56b39ad3439b92ca4c0",
        model: "BP60NB10",
        version: "1.02-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: true, // date 2121-07-08 (NB12 variant)
    },
    KnownFirmware {
        sha256: "221ad35b7edd402353e125841893ce651064e8fc6b90368fe84ff19a85a506f4",
        model: "BU40N",
        version: "1.00",
        form_factor: DriveFormFactor::Slim,
        is_mk: false,
        is_encrypted: false, // date 2116-12-20
    },
    KnownFirmware {
        sha256: "64900e8d69212f5d729b9dfa45ef0f317378db8c8b48083a7cb99831235b5d57",
        model: "BU40N",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: false, // date 2118-10-24
    },
    KnownFirmware {
        sha256: "592e8d22d98aea9c1162c9f010fe2df29b31f6320d539336a699a78ea3af9191",
        model: "WP50NB40",
        version: "1.03-MK",
        form_factor: DriveFormFactor::Slim,
        is_mk: true,
        is_encrypted: true, // date 2120-05-07
    },
];

/// Extract the PCB type from the boot string at offset 12288.
/// The boot string looks like "MT1959 Boot JB8 " or "MT1959 Boot BU5 ".
fn extract_pcb_type(data: &[u8]) -> Option<String> {
    const BOOT_OFFSET: usize = 12288;
    const BOOT_LEN: usize = 20; // "MT1959 Boot XXXX" = 20 bytes max
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
    // Search in the first 256KB where the model string is typically embedded.
    let search_region = &data[..data.len().min(256 * 1024)];
    // Single source of model names: platform classification tables.
    for model in crate::platform::known_models() {
        let model_bytes = model.as_bytes();
        if search_region
            .windows(model_bytes.len())
            .any(|w| w == model_bytes)
        {
            return Some(model.to_string());
        }
    }
    None
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
    let year = parse_u32(&s[0..4])?;
    let month = parse_u32(&s[4..6])?;
    let day = parse_u32(&s[6..8])?;
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

fn parse_u32(s: &str) -> Option<u32> {
    s.parse::<u32>().ok()
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
    let sha256 = crate::flash::sha256_hex(data);
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
    if id.binary_info.form_factor != DriveFormFactor::Unknown {
        return id.binary_info.form_factor;
    }
    DriveFormFactor::Unknown
}

/// Best-effort form factor determination with SDF0 metadata fallback.
/// Priority: known hash > binary PCB type > SDF0 model > Unknown.
pub fn resolve_form_factor_with_sdf(
    id: &FirmwareIdentification,
    sdf_info: Option<&crate::flash::FirmwareSdfInfo>,
) -> DriveFormFactor {
    let ff = resolve_form_factor(id);
    if ff != DriveFormFactor::Unknown {
        return ff;
    }
    if let Some(sdf) = sdf_info {
        if let Some(model) = &sdf.model {
            return crate::platform::classify_drive(model);
        }
    }
    DriveFormFactor::Unknown
}

/// Best-effort model determination.
/// Known firmware hash takes priority, then binary content, then SDF0 metadata.
pub fn resolve_model(id: &FirmwareIdentification) -> Option<String> {
    if let Some(known) = id.known {
        return Some(known.model.to_string());
    }
    id.binary_info.model.clone()
}

/// Best-effort model determination with SDF0 metadata fallback.
pub fn resolve_model_with_sdf(
    id: &FirmwareIdentification,
    sdf_info: Option<&crate::flash::FirmwareSdfInfo>,
) -> Option<String> {
    let model = resolve_model(id);
    if model.is_some() {
        return model;
    }
    sdf_info.and_then(|sdf| sdf.model.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!fw.is_mk);
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
        // "MT1959 Boot JB8 " at offset 12288
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
        // 12307 = BOOT_OFFSET + 19 — one byte short of the 20-byte slice.
        // Must not panic (regression test for off-by-one in length check).
        let data = vec![0u8; 12307];
        assert!(extract_pcb_type(&data).is_none());
    }

    #[test]
    fn extract_pcb_type_boundary_length_12308() {
        // 12308 = BOOT_OFFSET + 20 — exactly enough for the slice.
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
        // Real firmware binaries contain non-UTF-8 bytes (0xFF, etc.).
        // The byte-level search must still find the model string.
        let mut data = vec![0xFFu8; 40000]; // 0xFF is invalid UTF-8
        let model = b"BW-16D1HT";
        data[37600..37600 + model.len()].copy_from_slice(model);
        let found = extract_model(&data);
        assert_eq!(found.as_deref(), Some("BW-16D1HT"));
    }

    #[test]
    fn extract_model_works_with_mixed_binary_data() {
        // Firmware with a mix of valid and invalid UTF-8 bytes before the model.
        let mut data = vec![0u8; 40000];
        // Fill early region with non-UTF-8 bytes
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
        // Known says Desktop, binary says Slim — known wins
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
    fn extract_pcb_type_empty_after_trim() {
        // Boot string is "MT1959 Boot " followed by only spaces/nulls
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
        let sdf = crate::flash::FirmwareSdfInfo {
            vendor: Some("HL-DT-ST".to_string()),
            model: Some("BU40N".to_string()),
            firmware_version: Some("1.00".to_string()),
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
        let sdf = crate::flash::FirmwareSdfInfo {
            vendor: None,
            model: None,
            firmware_version: None,
        };
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
        let sdf = crate::flash::FirmwareSdfInfo {
            vendor: None,
            model: Some("ALSO_WRONG".to_string()),
            firmware_version: None,
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
        let sdf = crate::flash::FirmwareSdfInfo {
            vendor: None,
            model: Some("SHOULD_NOT_USE".to_string()),
            firmware_version: None,
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
        let sdf = crate::flash::FirmwareSdfInfo {
            vendor: Some("HL-DT-ST".to_string()),
            model: Some("BU40N".to_string()),
            firmware_version: Some("1.00".to_string()),
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
        let sdf = crate::flash::FirmwareSdfInfo {
            vendor: None,
            model: None,
            firmware_version: None,
        };
        assert!(resolve_model_with_sdf(&id, Some(&sdf)).is_none());
    }

    // === Firmware date extraction tests ===

    #[test]
    fn extract_firmware_date_finds_encrypted_date() {
        // Simulate a firmware binary with a 2120 date stamp embedded.
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
        // 10-digit date (YYMMDDHHMM) should also be accepted.
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
        // 9-digit runs are too short (minimum is 10).
        let mut data = vec![0u8; 1000];
        let digits = b"212005070";
        data[100..100 + digits.len()].copy_from_slice(digits);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_rejects_invalid_month() {
        let mut data = vec![0u8; 100_000];
        // Year 2120, month 13 — invalid.
        let date = b"212013070917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_rejects_invalid_day() {
        let mut data = vec![0u8; 100_000];
        // Year 2120, month 05, day 32 — invalid.
        let date = b"212005320917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_rejects_year_out_of_range() {
        let mut data = vec![0u8; 100_000];
        // Year 1990 — before 2000, not a plausible firmware date.
        let date = b"199005070917";
        data[50_000..50_000 + date.len()].copy_from_slice(date);
        assert!(extract_firmware_date_from_binary(&data).is_none());
    }

    #[test]
    fn extract_firmware_date_skips_short_digit_runs_finds_real_date() {
        let mut data = vec![0u8; 100_000];
        // A short 5-digit run followed by the real date.
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

    // === resolve_firmware_encrypted tests ===

    #[test]
    fn resolve_firmware_encrypted_known_encrypted() {
        // BP50NB40 1.03-MK hash — known to be encrypted (date 2120).
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
        // WH16NS60 1.02-MK hash — known to be non-encrypted (date 2118).
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
        // Exactly 4 known firmware files are encrypted (date ≥ 2120):
        // BP50NB40 1.03-MK, BP60NB10 1.02-MK (x2 variants), WP50NB40 1.03-MK.
        assert_eq!(encrypted.len(), 4);
        // All encrypted ones are slim external drives.
        assert!(encrypted
            .iter()
            .all(|(m, _)| m.starts_with("BP") || m.starts_with("WP")));
    }

    #[test]
    fn known_firmware_non_encrypted_flags_are_false() {
        let non_encrypted = KNOWN_FIRMWARES.iter().filter(|f| !f.is_encrypted).count();
        // 18 total - 4 encrypted = 14 non-encrypted.
        assert_eq!(non_encrypted, 14);
    }
}
