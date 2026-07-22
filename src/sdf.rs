use serde::{Deserialize, Serialize};
use std::io::{self, Read};

pub const SDF0_MAGIC: &[u8; 4] = b"SDF0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdfContainer {
    pub header: SdfHeader,
    pub metadata: SdfMetadata,
    pub payload: SdfPayload,
}

/// SDF0 binary header (big-endian, matching the sdf.bin format from makemkv.com):
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

    #[error("header_size {size} exceeds maximum {max}")]
    HeaderTooLarge { size: u32, max: u32 },

    #[error("header_size {0} is smaller than the minimum {SDF0_MIN_HEADER_SIZE}")]
    HeaderTooSmall(u32),

    #[error("metadata table exceeds maximum size of {max} bytes")]
    MetadataTooLarge { max: usize },
}

const SDF0_MIN_HEADER_SIZE: usize = 24;
const SDF0_MAX_HEADER_SIZE: u32 = 1024 * 1024;
const SDF0_MAX_METADATA_SIZE: usize = 1024 * 1024;
/// Upper bound for sane offset values. sdf.bin database files and firmware
/// containers are both well under 100 MB, so any offset beyond this is
/// encrypted data misinterpreted as a header field.
const SDF0_MAX_OFFSET: u32 = 100 * 1024 * 1024;

fn bytes4(buf: &[u8], offset: usize) -> Result<[u8; 4], SdfError> {
    buf.get(offset..offset + 4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(SdfError::DataTooShort {
            needed: offset + 4,
            have: buf.len(),
        })
}

fn be_u32(buf: &[u8], offset: usize) -> Result<u32, SdfError> {
    Ok(u32::from_be_bytes(bytes4(buf, offset)?))
}

fn skip_bytes<R: Read>(reader: &mut R, nbytes: usize) -> Result<(), SdfError> {
    if nbytes == 0 {
        return Ok(());
    }
    const CHUNK: usize = 8192;
    let mut buf = [0u8; CHUNK];
    let mut remaining = nbytes;
    while remaining > 0 {
        let take = remaining.min(CHUNK);
        reader.read_exact(&mut buf[..take])?;
        remaining -= take;
    }
    Ok(())
}

pub fn parse_sdf0<R: Read>(reader: &mut R) -> Result<SdfContainer, SdfError> {
    let mut header_buf = [0u8; SDF0_MIN_HEADER_SIZE];
    reader
        .read_exact(&mut header_buf)
        .map_err(|e| match e.kind() {
            io::ErrorKind::UnexpectedEof => SdfError::DataTooShort {
                needed: SDF0_MIN_HEADER_SIZE,
                have: 0,
            },
            _ => SdfError::Io(e),
        })?;

    let magic = bytes4(&header_buf, 0)?;
    if magic != *SDF0_MAGIC {
        return Err(SdfError::InvalidMagic {
            expected: *SDF0_MAGIC,
            got: magic,
        });
    }

    let version = be_u32(&header_buf, 4)?;
    if version == 0 {
        return Err(SdfError::InvalidVersion(version));
    }

    let header_size = be_u32(&header_buf, 8)?;
    let table_offset = be_u32(&header_buf, 12)?;
    let flags = be_u32(&header_buf, 16)?;
    let payload_offset = be_u32(&header_buf, 20)?;

    if !looks_like_structured_header(header_size, table_offset, payload_offset) {
        return Ok(minimal_sdf0_container(magic, version));
    }

    if header_size < SDF0_MIN_HEADER_SIZE as u32 {
        return Err(SdfError::HeaderTooSmall(header_size));
    }
    if header_size > SDF0_MAX_HEADER_SIZE {
        return Err(SdfError::HeaderTooLarge {
            size: header_size,
            max: SDF0_MAX_HEADER_SIZE,
        });
    }

    if header_size as usize > SDF0_MIN_HEADER_SIZE {
        let remaining = header_size as usize - SDF0_MIN_HEADER_SIZE;
        skip_bytes(reader, remaining)?;
    }

    let metadata_start = if table_offset == 0 {
        header_size
    } else {
        table_offset
    };

    let region_size = (payload_offset - header_size) as usize;
    if region_size > SDF0_MAX_METADATA_SIZE {
        return Err(SdfError::MetadataTooLarge {
            max: SDF0_MAX_METADATA_SIZE,
        });
    }

    if metadata_start > header_size {
        let gap = (metadata_start - header_size) as usize;
        skip_bytes(reader, gap)?;
    }

    let table_buf = read_metadata_table(reader, payload_offset - metadata_start)?;

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

/// Heuristic: check whether header offset fields look like a structured SDF0
/// firmware container rather than a sdf.bin database file (where offsets 8–23
/// are encrypted data).
fn looks_like_structured_header(header_size: u32, table_offset: u32, payload_offset: u32) -> bool {
    if header_size > SDF0_MAX_OFFSET {
        return false;
    }
    if payload_offset > SDF0_MAX_OFFSET {
        return false;
    }
    if table_offset > SDF0_MAX_OFFSET {
        return false;
    }
    if table_offset != 0 && table_offset < header_size {
        return false;
    }
    if payload_offset < header_size {
        return false;
    }
    if table_offset != 0 && table_offset > payload_offset {
        return false;
    }
    true
}

/// Build a minimal SDF0 container for sdf.bin database files: just magic +
/// version, with the entire payload marked as encrypted.
fn minimal_sdf0_container(magic: [u8; 4], version: u32) -> SdfContainer {
    SdfContainer {
        header: SdfHeader {
            magic,
            version,
            header_size: 8,
            table_offset: 0,
            flags: 0x01,
            payload_offset: 8,
        },
        metadata: SdfMetadata::default(),
        payload: SdfPayload {
            offset: 8,
            size: 0,
            encrypted: true,
            compressed: false,
        },
    }
}

fn read_metadata_table<R: Read>(reader: &mut R, size: u32) -> Result<Vec<u8>, SdfError> {
    let size = size as usize;
    if size > SDF0_MAX_METADATA_SIZE {
        return Err(SdfError::MetadataTooLarge {
            max: SDF0_MAX_METADATA_SIZE,
        });
    }
    if size == 0 {
        return Ok(Vec::new());
    }
    let mut table_buf = vec![0u8; size];
    reader.read_exact(&mut table_buf)?;
    Ok(table_buf)
}

fn parse_metadata_table(buf: &[u8]) -> Result<SdfMetadata, SdfError> {
    let mut metadata = SdfMetadata::default();
    let mut pos = 0;

    while pos + 1 < buf.len() {
        let key_end = match buf[pos..].iter().position(|&b| b == 0) {
            Some(p) => pos + p,
            None => break,
        };
        let key = match String::from_utf8(buf[pos..key_end].to_vec()) {
            Ok(key) => key,
            Err(_) => break,
        };
        if key.is_empty() {
            break;
        }
        pos = key_end + 1;

        if pos >= buf.len() {
            break;
        }

        let val_end = match buf[pos..].iter().position(|&b| b == 0) {
            Some(p) => pos + p,
            None => break,
        };
        let value = match String::from_utf8(buf[pos..val_end].to_vec()) {
            Ok(value) => value,
            Err(_) => break,
        };
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

/// Shared field view for CLI and GUI presentation (one place for field list).
#[derive(Debug, Clone, Copy)]
pub struct ContainerPresentation<'a> {
    pub version: u32,
    pub header_size: u32,
    pub table_offset: u32,
    pub flags: u32,
    pub payload_offset: u32,
    pub encrypted: bool,
    pub compressed: bool,
    pub vendor: Option<&'a str>,
    pub model: Option<&'a str>,
    pub firmware: Option<&'a str>,
    pub extra: &'a [(String, String)],
}

/// Extract presentation fields from a parsed container.
pub fn container_presentation(container: &SdfContainer) -> ContainerPresentation<'_> {
    ContainerPresentation {
        version: container.header.version,
        header_size: container.header.header_size,
        table_offset: container.header.table_offset,
        flags: container.header.flags,
        payload_offset: container.payload.offset,
        encrypted: container.payload.encrypted,
        compressed: container.payload.compressed,
        vendor: container.metadata.vendor.as_deref(),
        model: container.metadata.model.as_deref(),
        firmware: container.metadata.firmware_version.as_deref(),
        extra: container.metadata.extra.as_slice(),
    }
}

/// CLI text output for `sdf-info`.
pub fn format_container_cli(container: &SdfContainer, file: &str) -> String {
    let p = container_presentation(container);
    let mut out = format!("SDF0 Container: {file}\n");
    out.push_str(&format!("  Version:        {}\n", p.version));
    out.push_str(&format!("  Header size:    {}\n", p.header_size));
    out.push_str(&format!("  Table offset:   {}\n", p.table_offset));
    out.push_str(&format!("  Flags:          0x{:08x}\n", p.flags));
    out.push_str(&format!("  Payload offset: {}\n", p.payload_offset));
    out.push_str(&format!("  Encrypted:      {}\n", p.encrypted));
    out.push_str(&format!("  Compressed:     {}\n", p.compressed));
    if let Some(v) = p.vendor {
        out.push_str(&format!("  Vendor:         {v}\n"));
    }
    if let Some(m) = p.model {
        out.push_str(&format!("  Model:          {m}\n"));
    }
    if let Some(fw) = p.firmware {
        out.push_str(&format!("  Firmware:       {fw}\n"));
    }
    for (k, v) in p.extra {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    out
}

/// Localized log text for GUI settings parse button.
pub fn format_container_log(container: &SdfContainer, lang: crate::i18n::Language) -> String {
    use crate::i18n::{t_with_args, L10nKey};
    let p = container_presentation(container);
    let version = p.version.to_string();
    let header_size = p.header_size.to_string();
    let offset = p.payload_offset.to_string();
    let mut info = t_with_args(
        L10nKey::LogSdfHeader,
        lang,
        &[
            ("version", &version),
            ("header_size", &header_size),
            ("offset", &offset),
        ],
    );
    if let Some(v) = p.vendor {
        info.push('\n');
        info.push_str(&t_with_args(L10nKey::LogSdfVendor, lang, &[("vendor", v)]));
    }
    if let Some(m) = p.model {
        info.push('\n');
        info.push_str(&t_with_args(L10nKey::LogSdfModel, lang, &[("model", m)]));
    }
    if let Some(fw) = p.firmware {
        info.push('\n');
        info.push_str(&t_with_args(
            L10nKey::LogSdfFirmware,
            lang,
            &[("firmware", fw)],
        ));
    }
    let enc = p.encrypted.to_string();
    let comp = p.compressed.to_string();
    info.push('\n');
    info.push_str(&t_with_args(
        L10nKey::LogSdfFlags,
        lang,
        &[("encrypted", &enc), ("compressed", &comp)],
    ));
    for (k, v) in p.extra {
        info.push('\n');
        info.push_str(&t_with_args(
            L10nKey::LogSdfExtraField,
            lang,
            &[("key", k), ("value", v)],
        ));
    }
    info
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
        buf.extend_from_slice(&version.to_be_bytes());
        buf.extend_from_slice(&header_size.to_be_bytes());
        buf.extend_from_slice(&table_offset.to_be_bytes());
        buf.extend_from_slice(&flags.to_be_bytes());
        buf.extend_from_slice(&payload_offset.to_be_bytes());
        buf
    }

    fn build_sdf0_with_metadata(
        version: u32,
        flags: u32,
        metadata: &[u8],
        extra_payload: usize,
    ) -> Vec<u8> {
        let header_size = 24u32;
        let payload_offset = header_size + metadata.len() as u32;
        let mut data = build_sdf0_header(version, header_size, header_size, flags, payload_offset);
        data.extend_from_slice(metadata);
        if extra_payload > 0 {
            data.extend(vec![0u8; extra_payload]);
        }
        data
    }

    #[test]
    fn parse_valid_sdf0_header() {
        let data = build_sdf0_header(1, 24, 0, 0x01, 24);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).expect("should parse");
        assert_eq!(container.header.version, 1);
        assert!(container.payload.encrypted);
    }

    #[test]
    fn parse_sdf0_with_metadata() {
        let data = build_sdf0_with_metadata(2, 0x00, b"Vendor\0TestVendor\0Model\0BD-RW\0", 0);
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
        let data = vec![0u8; 10];
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
        let data = build_sdf0_header(1, 24, 0, 0x02, 24);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(!container.payload.encrypted);
        assert!(container.payload.compressed);
    }

    #[test]
    fn parse_sdf0_both_flags() {
        let data = build_sdf0_header(1, 24, 0, 0x03, 24);
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
        let data = build_sdf0_with_metadata(1, 0x00, b"Vendor\0LG\0Capabilities\0enc,boot\0", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.vendor.as_deref(), Some("LG"));
        assert_eq!(container.metadata.capabilities, vec!["enc", "boot"]);
    }

    #[test]
    fn parse_sdf0_metadata_fwver() {
        let data = build_sdf0_with_metadata(1, 0x00, b"FirmwareVersion\01.04\0", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.firmware_version.as_deref(), Some("1.04"));
    }

    #[test]
    fn parse_sdf0_empty_metadata() {
        let mut data = build_sdf0_header(1, 24, 0, 0x00, 28);
        data.extend_from_slice(&[0u8; 4]);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(container.metadata.vendor.is_none());
        assert!(container.metadata.model.is_none());
    }

    #[test]
    fn parse_sdf0_model_key() {
        let data = build_sdf0_with_metadata(1, 0x00, b"Model\0BU40N\0", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.model.as_deref(), Some("BU40N"));
    }

    #[test]
    fn parse_sdf0_model_lowercase() {
        let data = build_sdf0_with_metadata(1, 0x00, b"model\0BU40N\0", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.model.as_deref(), Some("BU40N"));
    }

    #[test]
    fn parse_sdf0_fwver_key() {
        let data = build_sdf0_with_metadata(1, 0x00, b"FWVer\01.04\0", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.firmware_version.as_deref(), Some("1.04"));
    }

    #[test]
    fn parse_sdf0_unknown_key_in_extra() {
        let data = build_sdf0_with_metadata(1, 0x00, b"CustomKey\0CustomVal\0", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.extra.len(), 1);
        assert_eq!(container.metadata.extra[0].0, "CustomKey");
        assert_eq!(container.metadata.extra[0].1, "CustomVal");
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_known_and_unknown_keys() {
        let data = build_sdf0_with_metadata(1, 0x00, b"Vendor\0LG\0Custom\0Val\0Model\0BU40N\0", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.vendor.as_deref(), Some("LG"));
        assert_eq!(container.metadata.model.as_deref(), Some("BU40N"));
        assert_eq!(container.metadata.extra.len(), 3);
    }

    #[test]
    fn parse_sdf0_truncated_metadata() {
        let data = build_sdf0_with_metadata(1, 0x00, b"Ven", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_truncated_value() {
        let data = build_sdf0_with_metadata(1, 0x00, b"Vendor\0LG", 0);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_rejects_oversized_header_size() {
        let oversize = SDF0_MAX_HEADER_SIZE + 1;
        let data = build_sdf0_header(1, oversize, 0, 0x00, oversize + 100);
        let mut cursor = Cursor::new(&data);
        let err = parse_sdf0(&mut cursor).unwrap_err();
        assert!(matches!(
            err,
            SdfError::HeaderTooLarge {
                size,
                max
            } if size == oversize && max == SDF0_MAX_HEADER_SIZE
        ));
    }

    #[test]
    fn parse_sdf0_rejects_header_size_smaller_than_minimum() {
        let data = build_sdf0_header(1, 16, 0, 0x00, 24);
        let mut cursor = Cursor::new(&data);
        let err = parse_sdf0(&mut cursor).unwrap_err();
        assert!(matches!(err, SdfError::HeaderTooSmall(16)));
    }

    #[test]
    fn parse_sdf0_rejects_oversized_metadata_table() {
        let metadata_bytes = SDF0_MAX_METADATA_SIZE + 1;
        let payload_offset = 24 + metadata_bytes as u32;
        let mut data = build_sdf0_header(1, 24, 24, 0x00, payload_offset);
        data.extend(vec![0u8; metadata_bytes]);
        let mut cursor = Cursor::new(&data);
        let err = parse_sdf0(&mut cursor).unwrap_err();
        assert!(matches!(
            err,
            SdfError::MetadataTooLarge { max } if max == SDF0_MAX_METADATA_SIZE
        ));
    }

    #[test]
    fn parse_sdf0_rejects_oversized_padding_before_metadata() {
        let header_size = 24u32;
        let table_offset = header_size + SDF0_MAX_METADATA_SIZE as u32;
        let payload_offset = table_offset + 4;
        let data = build_sdf0_header(1, header_size, table_offset, 0x00, payload_offset);
        let err = parse_sdf0(&mut Cursor::new(&data)).unwrap_err();
        assert!(matches!(
            err,
            SdfError::MetadataTooLarge { max } if max == SDF0_MAX_METADATA_SIZE
        ));
    }

    #[test]
    fn skip_bytes_zero_is_noop() {
        let mut cursor = Cursor::new(&[] as &[u8]);
        skip_bytes(&mut cursor, 0).expect("zero-byte skip");
    }

    #[test]
    fn skip_bytes_reads_large_gap_in_chunks() {
        let padding = vec![0xBBu8; 10_000];
        let mut cursor = Cursor::new(padding.as_slice());
        skip_bytes(&mut cursor, padding.len()).expect("chunked skip");
        assert_eq!(cursor.position(), padding.len() as u64);
    }

    #[test]
    fn read_metadata_table_rejects_oversized_buffer() {
        let mut cursor = Cursor::new(&[] as &[u8]);
        let err = read_metadata_table(&mut cursor, SDF0_MAX_METADATA_SIZE as u32 + 1).unwrap_err();
        assert!(matches!(
            err,
            SdfError::MetadataTooLarge { max } if max == SDF0_MAX_METADATA_SIZE
        ));
    }

    #[test]
    fn parse_sdf0_ignores_large_payload_after_metadata() {
        let data = build_sdf0_with_metadata(
            1,
            0x00,
            b"Vendor\0TestVendor\0Model\0TestModel\0",
            2 * 1024 * 1024,
        );
        let mut cursor = Cursor::new(&data);
        let container =
            parse_sdf0(&mut cursor).expect("should parse metadata without reading payload");
        assert_eq!(container.metadata.vendor.as_deref(), Some("TestVendor"));
        assert_eq!(container.metadata.model.as_deref(), Some("TestModel"));
    }

    #[test]
    fn parse_sdf0_padded_metadata_region_with_table_gap() {
        let metadata = b"Vendor\0TestVendor\0Model\0TestModel\0";
        let header_size = 24u32;
        let table_offset = 32u32;
        let payload_offset = 128u32;
        let mut data = build_sdf0_header(1, header_size, table_offset, 0x00, payload_offset);
        data.extend_from_slice(&[0x00; 8]);
        data.extend_from_slice(metadata);
        data.resize(payload_offset as usize, 0xAA);
        data.extend(vec![0u8; 64]);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).expect("should parse padded metadata");
        assert_eq!(container.metadata.vendor.as_deref(), Some("TestVendor"));
        assert_eq!(container.metadata.model.as_deref(), Some("TestModel"));
    }

    #[test]
    fn parse_sdf0_larger_header() {
        let metadata = b"Vendor\0Test\0";
        let header_size = 32u32;
        let payload_offset = header_size + metadata.len() as u32;
        let mut data = build_sdf0_header(1, header_size, header_size, 0x00, payload_offset);
        data.extend_from_slice(&[0xAA; 8]);
        data.extend_from_slice(metadata);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert_eq!(container.metadata.vendor.as_deref(), Some("Test"));
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
        }
    }

    #[test]
    fn parse_sdf0_io_error() {
        let mut reader = FailingReader;
        let err = parse_sdf0(&mut reader).unwrap_err();
        assert!(matches!(err, SdfError::Io(_)));
    }

    #[test]
    fn parse_metadata_key_no_null() {
        let data = b"Vendor";
        let metadata = parse_metadata_table(data).unwrap();
        assert!(metadata.vendor.is_none());
    }

    #[test]
    fn parse_metadata_key_then_no_room_for_value() {
        let data = b"Ven\0";
        let metadata = parse_metadata_table(data).unwrap();
        assert!(metadata.vendor.is_none());
    }

    #[test]
    fn parse_metadata_value_no_null() {
        let data = b"Vendor\0Test";
        let metadata = parse_metadata_table(data).unwrap();
        assert!(metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_zero_table_offset() {
        let data = build_sdf0_header(1, 24, 0, 0x00, 24);
        let mut cursor = Cursor::new(&data);
        let container = parse_sdf0(&mut cursor).unwrap();
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_inconsistent_offsets_falls_back_to_minimal() {
        let data = build_sdf0_header(1, 24, 24, 0x00, 20);
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        assert_eq!(container.header.version, 1);
        assert_eq!(container.header.header_size, 8);
        assert!(container.payload.encrypted);
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_table_after_payload_falls_back_to_minimal() {
        let data = build_sdf0_header(1, 24, 28, 0x00, 26);
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        assert_eq!(container.header.version, 1);
        assert_eq!(container.header.header_size, 8);
        assert!(container.payload.encrypted);
    }

    #[test]
    fn parse_sdf0_table_before_header_falls_back_to_minimal() {
        let data = build_sdf0_header(1, 24, 16, 0x00, 24);
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        assert_eq!(container.header.version, 1);
        assert_eq!(container.header.header_size, 8);
        assert!(container.payload.encrypted);
    }

    #[test]
    fn format_container_cli_includes_metadata() {
        let data = build_sdf0_with_metadata(
            1,
            0x01,
            b"Vendor\0LG\0Model\0BU40N\0FirmwareVersion\01.04\0CustomKey\0CustomVal\0",
            0,
        );
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        let text = format_container_cli(&container, "fw.bin");
        assert!(text.contains("fw.bin"));
        assert!(text.contains("Vendor:         LG"));
        assert!(text.contains("Model:          BU40N"));
        assert!(text.contains("Firmware:       1.04"));
        assert!(text.contains("CustomKey: CustomVal"));
        assert!(text.contains("Encrypted:      true"));
    }

    #[test]
    fn format_container_log_includes_localized_metadata() {
        let data = build_sdf0_with_metadata(
            1,
            0x00,
            b"Vendor\0LG\0Model\0BU40N\0FirmwareVersion\01.04\0",
            0,
        );
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        let text = format_container_log(&container, crate::i18n::Language::German);
        assert!(text.contains("LG"));
        assert!(text.contains("BU40N"));
        assert!(text.contains("1.04"));
    }

    #[test]
    fn parse_sdf0_metadata_invalid_utf8_key() {
        let mut metadata = vec![0xFF, 0xFE, 0x00, b'L', b'G', 0x00];
        metadata.extend_from_slice(b"Vendor\0LG\0");
        let data = build_sdf0_with_metadata(1, 0x00, &metadata, 0);
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_metadata_empty_key_stops_parsing() {
        let data = build_sdf0_with_metadata(1, 0x00, b"\0Vendor\0LG\0", 0);
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn parse_sdf0_metadata_invalid_utf8_value() {
        let mut metadata = b"Vendor\0".to_vec();
        metadata.extend_from_slice(&[0xFF, 0xFE]);
        metadata.push(0x00);
        let data = build_sdf0_with_metadata(1, 0x00, &metadata, 0);
        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn sdf0_magic_constant() {
        assert_eq!(SDF0_MAGIC, b"SDF0");
    }

    #[test]
    fn sdf_error_display() {
        let err = SdfError::InvalidMagic {
            expected: *SDF0_MAGIC,
            got: *b"NOPE",
        };
        let msg = format!("{err}");
        assert!(msg.contains("SDF0"));
        assert!(msg.contains("invalid SDF0 magic"));
    }

    #[test]
    fn sdf_error_data_too_short_display() {
        let err = SdfError::DataTooShort {
            needed: 24,
            have: 10,
        };
        let msg = format!("{err}");
        assert!(msg.contains("24"));
        assert!(msg.contains("10"));
    }

    #[test]
    fn sdf_error_invalid_version_display() {
        let err = SdfError::InvalidVersion(0);
        let msg = format!("{err}");
        assert!(msg.contains("0"));
    }

    #[test]
    fn parse_sdf0_database_file_minimal_container() {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&0xa6u32.to_be_bytes());
        data.extend_from_slice(&0x00000408u32.to_be_bytes());
        data.extend_from_slice(&0x8001f936u32.to_be_bytes());
        data.extend_from_slice(&0x000f000cu32.to_be_bytes());
        data.extend_from_slice(&0x1a000102u32.to_be_bytes());
        data.extend(vec![0xAB; 100]);

        let container = parse_sdf0(&mut Cursor::new(&data)).unwrap();
        assert_eq!(container.header.version, 166);
        assert_eq!(container.header.header_size, 8);
        assert_eq!(container.header.payload_offset, 8);
        assert!(container.payload.encrypted);
        assert!(container.metadata.vendor.is_none());
    }

    #[test]
    fn looks_like_structured_header_valid() {
        assert!(looks_like_structured_header(24, 0, 24));
        assert!(looks_like_structured_header(24, 24, 48));
        assert!(looks_like_structured_header(32, 32, 64));
    }

    #[test]
    fn looks_like_structured_header_invalid() {
        assert!(!looks_like_structured_header(24, 0, SDF0_MAX_OFFSET + 1));
        assert!(!looks_like_structured_header(SDF0_MAX_OFFSET + 1, 0, 24,));
        assert!(!looks_like_structured_header(24, 24, 20));
        assert!(!looks_like_structured_header(24, 16, 24));
        assert!(!looks_like_structured_header(24, 48, 32));
    }
    #[test]
    fn looks_like_structured_header_table_offset_too_large() {
        assert!(!looks_like_structured_header(24, SDF0_MAX_OFFSET + 1, 100));
    }

    #[test]
    fn parse_sdf0_header_size_larger_than_min_skips_padding() {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&32u32.to_be_bytes());
        data.extend_from_slice(&32u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&40u32.to_be_bytes());
        data.extend_from_slice(&[0u8; 8]);
        data.extend_from_slice(b"Vendor\0X\0");
        data.resize(40, 0);
        let mut cursor = std::io::Cursor::new(&data);
        let c = parse_sdf0(&mut cursor).expect("parse");
        assert_eq!(c.header.header_size, 32);
    }

    #[test]
    fn parse_sdf0_rejects_payload_before_header() {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&50u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&40u32.to_be_bytes());
        let mut cursor = std::io::Cursor::new(&data);
        let c = parse_sdf0(&mut cursor).expect("fallback");
        assert!(c.metadata.vendor.is_none());
    }
}
