use serde::{Deserialize, Serialize};

#[allow(dead_code)]
pub const FIRMWARE_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareManifest {
    pub schema_version: u32,
    pub vendor: String,
    pub model: String,
    pub revision_match: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Optional drive category hint (e.g. "internal", "slim").
    /// Used for warnings only — never blocks a flash operation.
    #[serde(default)]
    pub category: Option<String>,
    pub firmware_images: Vec<FirmwareImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareImage {
    pub image_id: String,
    pub filename: String,
    pub target_version: String,
    pub size: u64,
    /// SHA-256 hex digest of the **entire firmware file** on disk (not an extracted sub-image payload).
    /// Multi-image packs must hash the full `.bin` file that will be passed to the backend.
    pub sha256: String,
    #[serde(default)]
    pub signature_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveMatch {
    pub vendor: String,
    pub model: String,
    pub revision: String,
}

pub fn parse_manifest(data: &[u8]) -> Result<FirmwareManifest, serde_json::Error> {
    serde_json::from_slice(data)
}

pub fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return value.ends_with(suffix);
    }
    pattern.eq_ignore_ascii_case(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("BU40N", "BU40N"));
        assert!(!glob_match("BU40N", "BU40N2"));
    }

    #[test]
    fn glob_match_prefix() {
        assert!(glob_match("3.1*", "3.10"));
        assert!(glob_match("3.1*", "3.11"));
        assert!(!glob_match("3.1*", "3.02"));
    }

    #[test]
    fn glob_match_suffix() {
        assert!(glob_match("*N", "BU40N"));
        assert!(!glob_match("*N", "BU40X"));
    }

    #[test]
    fn glob_match_wildcard() {
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn glob_match_case_insensitive() {
        assert!(glob_match("BU40N", "bu40n"));
        assert!(glob_match("BU40N", "BU40N"));
    }

    #[test]
    fn parse_manifest_valid() {
        let json = r#"{
            "schema_version": 1,
            "vendor": "HL-DT-ST",
            "model": "BU40N",
            "revision_match": "1.0*",
            "firmware_images": [{
                "image_id": "main",
                "filename": "fw.bin",
                "target_version": "1.04",
                "size": 1024,
                "sha256": "abcd"
            }]
        }"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(manifest.vendor, "HL-DT-ST");
        assert_eq!(manifest.model, "BU40N");
        assert_eq!(manifest.firmware_images.len(), 1);
        assert_eq!(manifest.firmware_images[0].image_id, "main");
        assert!(!manifest.firmware_images[0].signature_present); // default
    }

    #[test]
    fn parse_manifest_invalid_json() {
        assert!(parse_manifest(b"not json").is_err());
    }

    #[test]
    fn parse_manifest_missing_required_field() {
        let json = r#"{"schema_version": 1}"#;
        assert!(parse_manifest(json.as_bytes()).is_err());
    }

    #[test]
    fn parse_manifest_with_capabilities() {
        let json = r#"{
            "schema_version": 1,
            "vendor": "V",
            "model": "M",
            "revision_match": "*",
            "capabilities": ["enc", "boot"],
            "firmware_images": []
        }"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(manifest.capabilities, vec!["enc", "boot"]);
    }

    #[test]
    fn glob_match_empty_pattern() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "a"));
    }

    #[test]
    fn glob_match_prefix_no_match() {
        assert!(!glob_match("BU*", "LG40N"));
    }

    #[test]
    fn glob_match_suffix_no_match() {
        assert!(!glob_match("*N", "BU40X"));
    }

    #[test]
    fn glob_match_single_char_prefix() {
        assert!(glob_match("1*", "1.03"));
        assert!(!glob_match("1*", "2.03"));
    }

    #[test]
    fn glob_match_case_sensitive_prefix() {
        // Prefix matching is case-sensitive
        assert!(glob_match("BU*", "BU40N"));
        assert!(!glob_match("bu*", "BU40N"));
    }

    #[test]
    fn glob_match_case_sensitive_suffix() {
        // Suffix matching is case-sensitive
        assert!(glob_match("*N", "BU40N"));
        assert!(!glob_match("*n", "BU40N"));
    }

    #[test]
    fn parse_manifest_signature_present() {
        let json = r#"{
            "schema_version": 1,
            "vendor": "HL-DT-ST",
            "model": "BU40N",
            "revision_match": "*",
            "firmware_images": [{
                "image_id": "main",
                "filename": "fw.bin",
                "target_version": "1.04",
                "size": 1024,
                "sha256": "abcd",
                "signature_present": true
            }]
        }"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert!(manifest.firmware_images[0].signature_present);
    }

    #[test]
    fn parse_manifest_empty_images() {
        let json = r#"{
            "schema_version": 1,
            "vendor": "V",
            "model": "M",
            "revision_match": "*",
            "firmware_images": []
        }"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert!(manifest.firmware_images.is_empty());
    }

    #[test]
    fn parse_manifest_multiple_images() {
        let json = r#"{
            "schema_version": 1,
            "vendor": "V",
            "model": "M",
            "revision_match": "*",
            "firmware_images": [
                {"image_id": "a", "filename": "a.bin", "target_version": "1.0", "size": 100, "sha256": "aa"},
                {"image_id": "b", "filename": "b.bin", "target_version": "2.0", "size": 200, "sha256": "bb"}
            ]
        }"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(manifest.firmware_images.len(), 2);
        assert_eq!(manifest.firmware_images[0].image_id, "a");
        assert_eq!(manifest.firmware_images[1].image_id, "b");
    }

    #[test]
    fn drive_match_from_drive() {
        use crate::drive::Drive;
        let drive = Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
        };
        let dm: DriveMatch = (&drive).into();
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BU40N");
        assert_eq!(dm.revision, "1.03");
    }

    #[test]
    fn parse_manifest_category_optional() {
        // Old manifests without "category" should parse with category = None
        let json = r#"{
            "schema_version": 1,
            "vendor": "V",
            "model": "M",
            "revision_match": "*",
            "firmware_images": []
        }"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert!(manifest.category.is_none());
    }

    #[test]
    fn parse_manifest_category_present() {
        let json = r#"{
            "schema_version": 1,
            "vendor": "V",
            "model": "M",
            "revision_match": "*",
            "category": "internal",
            "firmware_images": []
        }"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(manifest.category.as_deref(), Some("internal"));
    }
}
