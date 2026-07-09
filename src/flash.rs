use crate::sdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashDirection {
    Upgrade,
    Downgrade,
    Same,
}

pub fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    let mut hex = String::with_capacity(64);
    for b in hash {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Metadata extracted from a firmware binary's SDF0 header (if parseable).
#[derive(Debug, Clone)]
pub struct FirmwareSdfInfo {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
}

/// Try to parse the firmware binary as an SDF0 container and extract metadata.
/// Returns `None` if the binary is not a valid SDF0 container (e.g. encrypted
/// raw blobs), which is a common and expected case.
pub fn check_firmware_sdf(firmware_data: &[u8]) -> Option<FirmwareSdfInfo> {
    let mut cursor = std::io::Cursor::new(firmware_data);
    let container = sdf::parse_sdf0(&mut cursor).ok()?;
    Some(FirmwareSdfInfo {
        vendor: container.metadata.vendor,
        model: container.metadata.model,
        firmware_version: container.metadata.firmware_version,
    })
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
    // At least one version contains non-numeric segments; fall back to lexicographic.
    if current < target {
        FlashDirection::Upgrade
    } else {
        FlashDirection::Downgrade
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_known_input() {
        // SHA-256 of empty input
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
        // "1.0" and "1.00" differ as strings but parse to [1, 0] vs [1, 0].
        // Exercises the post-loop return (line 190).
        assert_eq!(compare_versions("1.0", "1.00"), FlashDirection::Same);
    }

    #[test]
    fn compare_versions_different_lengths() {
        assert_eq!(compare_versions("1.0", "1.0.1"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("1.0.1", "1.0"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_non_numeric_fallback() {
        // Falls back to string comparison when parts aren't numeric
        assert_eq!(compare_versions("abc", "def"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("def", "abc"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_mixed_numeric_non_numeric() {
        // One parses as numeric, the other doesn't — falls back to string comparison
        assert_eq!(compare_versions("1.0", "abc"), FlashDirection::Upgrade);
    }

    #[test]
    fn check_firmware_sdf_non_sdf_data() {
        // Encrypted raw blobs are not SDF0 — should return None
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(&[0x85, 0x4a, 0xc0, 0x75]);
        assert!(check_firmware_sdf(&data).is_none());
    }

    #[test]
    fn check_firmware_sdf_valid_sdf0() {
        // Build a minimal SDF0 container with metadata
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes()); // version
        data.extend_from_slice(&24u32.to_be_bytes()); // header_size
        data.extend_from_slice(&24u32.to_be_bytes()); // table_offset
        data.extend_from_slice(&0u32.to_be_bytes()); // flags
        let metadata = b"Vendor\0TestVendor\0Model\0TestModel\0";
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_be_bytes());
        data.extend_from_slice(metadata);

        let info = check_firmware_sdf(&data).unwrap();
        assert_eq!(info.vendor.as_deref(), Some("TestVendor"));
        assert_eq!(info.model.as_deref(), Some("TestModel"));
    }

    fn build_sdf0_firmware_bytes(vendor: &str, model: &str) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        let metadata = format!("Vendor\0{vendor}\0Model\0{model}\0");
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_be_bytes());
        data.extend_from_slice(metadata.as_bytes());
        data
    }

    #[test]
    fn check_firmware_sdf_extracts_vendor_and_model() {
        let firmware = build_sdf0_firmware_bytes("OtherVendor", "BU40N");
        let info = check_firmware_sdf(&firmware).expect("sdf metadata");
        assert_eq!(info.vendor.as_deref(), Some("OtherVendor"));
        assert_eq!(info.model.as_deref(), Some("BU40N"));
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
        assert_eq!(info.vendor.as_deref(), Some("TestVendor"));
        assert_eq!(info.model.as_deref(), Some("TestModel"));
    }
}
