// SDF0 container parser.
//
// Parses SDF0 binary headers and metadata from optical drive firmware
// containers. The payload is always reported as opaque/encrypted.

use serde::{Deserialize, Serialize};
use std::io::{self, Read};

pub const SDF0_MAGIC: &[u8; 4] = b"SDF0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdfContainer {
    pub header: SdfHeader,
    pub metadata: SdfMetadata,
    pub payload: SdfPayload,
}

/// SDF0 binary header (little-endian):
/// ```text
/// Offset  Size  Field
/// 0       4     magic ("SDF0")
/// 4       4     version
/// 8       4     header_size
/// 12      4     table_offset
/// 16      4     flags
/// 20      4     payload_offset
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdfHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub header_size: u32,
    pub table_offset: u32,
    pub flags: u32,
    pub payload_offset: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SdfMetadata {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub capabilities: Vec<String>,
    pub extra: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdfPayload {
    pub offset: u32,
    pub size: u32,
    pub encrypted: bool,
    pub compressed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SdfError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid SDF0 magic: expected {expected:?}, got {got:?}")]
    InvalidMagic { expected: [u8; 4], got: [u8; 4] },

    #[error("data too short: need at least {needed} bytes, have {have}")]
    DataTooShort { needed: usize, have: usize },

    #[error("invalid SDF0 version: {0}")]
    InvalidVersion(u32),

    #[error("invalid UTF-8 in metadata string: {0}")]
    InvalidString(#[from] std::string::FromUtf8Error),
}

const SDF0_MIN_HEADER_SIZE: usize = 24;

pub fn parse_sdf0<R: Read>(reader: &mut R) -> Result<SdfContainer, SdfError> {
    let mut header_buf = vec![0u8; SDF0_MIN_HEADER_SIZE];
    reader
        .read_exact(&mut header_buf)
        .map_err(|e| match e.kind() {
            io::ErrorKind::UnexpectedEof => SdfError::DataTooShort {
                needed: SDF0_MIN_HEADER_SIZE,
                have: 0,
            },
            _ => SdfError::Io(e),
        })?;

    let magic: [u8; 4] = [header_buf[0], header_buf[1], header_buf[2], header_buf[3]];
    if magic != *SDF0_MAGIC {
        return Err(SdfError::InvalidMagic {
            expected: *SDF0_MAGIC,
            got: magic,
        });
    }

    let version = u32::from_le_bytes(header_buf[4..8].try_into().unwrap());
    let header_size = u32::from_le_bytes(header_buf[8..12].try_into().unwrap());
    let table_offset = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());
    let flags = u32::from_le_bytes(header_buf[16..20].try_into().unwrap());
    let payload_offset = u32::from_le_bytes(header_buf[20..24].try_into().unwrap());

    if version == 0 {
        return Err(SdfError::InvalidVersion(version));
    }

    if header_size as usize > SDF0_MIN_HEADER_SIZE {
        let remaining = header_size as usize - SDF0_MIN_HEADER_SIZE;
        let mut skip_buf = vec![0u8; remaining];
        reader.read_exact(&mut skip_buf)?;
    }

    let mut table_buf = Vec::new();
    reader.read_to_end(&mut table_buf)?;

    let metadata = if table_buf.len() > 4 {
        parse_metadata_table(&table_buf)?
    } else {
        SdfMetadata::default()
    };

    let payload_encrypted = (flags & 0x01) != 0;
    let payload_compressed = (flags & 0x02) != 0;

    Ok(SdfContainer {
        header: SdfHeader {
            magic,
            version,
            header_size,
            table_offset,
            flags,
            payload_offset,
        },
        metadata,
        payload: SdfPayload {
            offset: payload_offset,
            size: 0,
            encrypted: payload_encrypted,
            compressed: payload_compressed,
        },
    })
}

fn parse_metadata_table(buf: &[u8]) -> Result<SdfMetadata, SdfError> {
    let mut metadata = SdfMetadata::default();
    let mut pos = 0;

    while pos + 1 < buf.len() {
        let key_end = match buf[pos..].iter().position(|&b| b == 0) {
            Some(p) => pos + p,
            None => break,
        };
        let key = String::from_utf8(buf[pos..key_end].to_vec())?;
        pos = key_end + 1;

        if pos >= buf.len() {
            break;
        }

        let val_end = match buf[pos..].iter().position(|&b| b == 0) {
            Some(p) => pos + p,
            None => break,
        };
        let value = String::from_utf8(buf[pos..val_end].to_vec())?;
        pos = val_end + 1;

        match key.as_str() {
            "Vendor" | "vendor" => metadata.vendor = Some(value.clone()),
            "Model" | "model" => metadata.model = Some(value.clone()),
            "FirmwareVersion" | "firmware_version" | "FWVer" => {
                metadata.firmware_version = Some(value.clone())
            }
            "Capabilities" | "capabilities" => {
                metadata.capabilities = value.split(',').map(|s| s.trim().to_string()).collect();
            }
            _ => {}
        }
        metadata.extra.push((key, value));
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn build_sdf0_header(
        version: u32,
        header_size: u32,
        table_offset: u32,
        flags: u32,
        payload_offset: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(SDF0_MAGIC);
        buf.extend_from_slice(&version.to_le_bytes());
        buf.extend_from_slice(&header_size.to_le_bytes());
        buf.extend_from_slice(&table_offset.to_le_bytes());
        buf.extend_from_slice(&flags.to_le_bytes());
        buf.extend_from_slice(&payload_offset.to_le_bytes());
        buf
    }

    #[test]
    fn parse_valid_sdf0_header() {
        let data = build_sdf0_header(1, 24, 0, 0x01, 48);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).expect("should parse");
        assert_eq!(container.header.version, 1);
        assert!(container.payload.encrypted);
    }

    #[test]
    fn parse_sdf0_with_metadata() {
        let mut data = build_sdf0_header(2, 24, 24, 0x00, 56);
        data.extend_from_slice(b"Vendor\0TestVendor\0Model\0BD-RW\0");
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).expect("should parse");
        assert_eq!(container.metadata.vendor.as_deref(), Some("TestVendor"));
    }

    #[test]
    fn parse_sdf0_invalid_magic() {
        let mut data = b"NOPE".to_vec();
        data.extend_from_slice(&[0u8; 20]);
        let mut cursor = Cursor::new(&data);
        let err = parse_sdf0(&mut cursor).unwrap_err();
        assert!(matches!(err, SdfError::InvalidMagic { .. }));
    }

    #[test]
    fn parse_sdf0_data_too_short() {
        let data = vec![0u8; 10]; // less than 24 bytes
        let mut cursor = Cursor::new(&data);
        let err = parse_sdf0(&mut cursor).unwrap_err();
        assert!(matches!(err, SdfError::DataTooShort { .. }));
    }

    #[test]
    fn parse_sdf0_invalid_version() {
        let data = build_sdf0_header(0, 24, 0, 0, 24);
        let mut cursor = Cursor::new(&data);
        let err = parse_sdf0(&mut cursor).unwrap_err();
        assert!(matches!(err, SdfError::InvalidVersion(0)));
    }

    #[test]
    fn parse_sdf0_compressed_flag() {
        let data = build_sdf0_header(1, 24, 0, 0x02, 24); // flag bit 1 = compressed
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(!container.payload.encrypted);
        assert!(container.payload.compressed);
    }

    #[test]
    fn parse_sdf0_both_flags() {
        let data = build_sdf0_header(1, 24, 0, 0x03, 24); // encrypted + compressed
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(container.payload.encrypted);
        assert!(container.payload.compressed);
    }

    #[test]
    fn parse_sdf0_no_flags() {
        let data = build_sdf0_header(1, 24, 0, 0x00, 24);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(!container.payload.encrypted);
        assert!(!container.payload.compressed);
    }

    #[test]
    fn parse_sdf0_metadata_with_capabilities() {
        let mut data = build_sdf0_header(1, 24, 24, 0x00, 64);
        data.extend_from_slice(b"Vendor\0LG\0Capabilities\0enc,boot\0");
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.vendor.as_deref(), Some("LG"));
        assert_eq!(container.metadata.capabilities, vec!["enc", "boot"]);
    }

    #[test]
    fn parse_sdf0_metadata_fwver() {
        let mut data = build_sdf0_header(1, 24, 24, 0x00, 60);
        data.extend_from_slice(b"FirmwareVersion\01.04\0");
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.firmware_version.as_deref(), Some("1.04"));
    }

    #[test]
    fn parse_sdf0_empty_metadata() {
        // Table area is <= 4 bytes, so metadata defaults
        let data = build_sdf0_header(1, 24, 0, 0x00, 28);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(container.metadata.vendor.is_none());
        assert!(container.metadata.model.is_none());
    }
}
