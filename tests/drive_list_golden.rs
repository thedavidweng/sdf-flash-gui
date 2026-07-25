//! Cross-platform `sdftool -l` / `makemkvcon f -l` golden fixtures.
//!
//! These tests always run in CI (no optical hardware, no real sdftool binary).
//! They guard the list → parse → device-path pipeline that the GUI and CLI use
//! for drive selection.
//!
//! ## Why `e2e_firmware_pack` did not catch the macOS "no drives" bug
//!
//! 1. **Wrong scope** — `e2e_firmware_pack` exercises firmware filename/SHA
//!    logic against a private pack; it never calls `parse_drive_list` or
//!    `enumerate_drives`.
//! 2. **`#[ignore]`** — those tests are skipped in CI unless run with
//!    `--ignored` and a local firmware pack path.
//! 3. **Mocks used Linux one-liners** — unit tests mocked
//!    `0:/dev/sr0 HL-DT-ST BU40N 1.03\n` only. Real MakeMKV output is multi-line
//!    with OS-specific device ids (`/IOBDServices/…`, `E:`, `\\.\D:`, `/dev/sr0`).
//! 4. **No hardware in CI** — runners have no optical drives, so OS enumeration
//!    always returns empty; a broken multi-line parser still "passes".
//! 5. **`src/drive.rs` is coverage-ignored** — Codecov's 100% patch gate does
//!    not require new enumerate/parse branches to be covered.
//!
//! Keep these fixtures updated when adding a supported device path shape.

use sdf_flash_gui::drive::parse_drive_list;

/// Real capture shape from macOS MakeMKV with USB BD (no media).
const FIXTURE_MACOS_USB_BD: &str = "\
Found 1 drives(s)
00: /IOBDServices/F49D28A7
  HL-DT-ST_BD-RE_BU50N_GE03_211904231648_MODJ9TK3546

";

/// Linux SCSI CD-ROM node with multi-line identity (MakeMKV style).
const FIXTURE_LINUX_SR0: &str = "\
Found 1 drives(s)
00: /dev/sr0
  HL-DT-ST_BD-RE_BU40N_1.03_211904231648_MODJ9TK3546

";

/// Windows drive-letter form used by SPTI / MakeMKV docs (`-d E:`).
const FIXTURE_WINDOWS_LETTER: &str = "\
Found 1 drives(s)
00: E:
  HL-DT-ST_BD-RE_BU50N_GE03_211904231648_MODJ9TK3546

";

/// Windows extended device path.
const FIXTURE_WINDOWS_EXTENDED: &str = "\
Found 1 drives(s)
00: \\\\.\\D:
  ASUS_BW-16D1HT_3.10_SERIAL1234567_X

";

#[test]
fn golden_macos_iokit_path() {
    let drives = parse_drive_list(FIXTURE_MACOS_USB_BD);
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].device, "/IOBDServices/F49D28A7");
    assert_eq!(drives[0].vendor, "HL-DT-ST");
    assert_eq!(drives[0].product, "BD-RE BU50N");
    assert_eq!(drives[0].revision, "GE03");
    assert_eq!(drives[0].serial, "MODJ9TK3546");
    assert_eq!(drives[0].firmware_date, "211904231648");
    assert_eq!(drives[0].firmware_date_display(), "2119-04-23 16:48");
}

#[test]
fn golden_linux_sr_device() {
    let drives = parse_drive_list(FIXTURE_LINUX_SR0);
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].device, "/dev/sr0");
    assert_eq!(drives[0].vendor, "HL-DT-ST");
    assert_eq!(drives[0].product, "BD-RE BU40N");
    assert_eq!(drives[0].revision, "1.03");
}

#[test]
fn golden_windows_drive_letter() {
    let drives = parse_drive_list(FIXTURE_WINDOWS_LETTER);
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].device, "E:");
    assert_eq!(drives[0].vendor, "HL-DT-ST");
    assert_eq!(drives[0].product, "BD-RE BU50N");
    assert_eq!(drives[0].revision, "GE03");
}

#[test]
fn golden_windows_extended_path() {
    let drives = parse_drive_list(FIXTURE_WINDOWS_EXTENDED);
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].device, r"\\.\D:");
    assert_eq!(drives[0].vendor, "ASUS");
    assert_eq!(drives[0].product, "BW-16D1HT");
    assert_eq!(drives[0].revision, "3.10");
}

#[test]
fn golden_all_platforms_in_one_list() {
    let combined = format!(
        "Found 3 drives(s)\n{}\n{}\n{}\n",
        "00: /dev/sr0\n  HL-DT-ST_BD-RE_BU40N_1.03_SERIALAAAAAA_X",
        "01: E:\n  HL-DT-ST_BD-RE_BU50N_GE03_SERIALBBBBBB_Y",
        "02: /IOBDServices/F49D28A7\n  HL-DT-ST_BD-RE_BU50N_GE03_SERIALCCCCCC_Z",
    );
    let drives = parse_drive_list(&combined);
    assert_eq!(
        drives.len(),
        3,
        "must parse Linux + Windows + macOS paths together"
    );
    assert_eq!(drives[0].device, "/dev/sr0");
    assert_eq!(drives[1].device, "E:");
    assert_eq!(drives[2].device, "/IOBDServices/F49D28A7");
}

#[test]
fn golden_open_error_keeps_device_for_retry() {
    let output = "\
Found 1 drives(s)
00: /dev/sr0
  open error
";
    let drives = parse_drive_list(output);
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].device, "/dev/sr0");
}

#[test]
fn regression_no_false_device_zero_colon() {
    let drives = parse_drive_list(FIXTURE_MACOS_USB_BD);
    assert_eq!(drives.len(), 1);
    assert_ne!(drives[0].device, "0:");
    assert_ne!(drives[0].device, "00:");
}

/// MakeMKV RE / 01-sdftool-spec format B: inquiry on the index line.
#[test]
fn golden_format_b_inquiry_then_device() {
    let output = "\
Found 2 drives(s)
00:  HL-DT-ST BD-RE BU50N GE03
  /IOBDServices/F49D28A7
01:  HL-DT-ST BU40N 1.03
  open error
";
    let drives = parse_drive_list(output);
    assert_eq!(drives.len(), 2);
    assert_eq!(drives[0].device, "/IOBDServices/F49D28A7");
    assert_eq!(drives[0].revision, "GE03");
    assert_eq!(drives[1].vendor, "HL-DT-ST");
    assert_eq!(drives[1].device, "HL-DT-ST_BU40N_1.03");
}

#[test]
fn golden_resolve_selection_after_reenumeration() {
    use sdf_flash_gui::drive::{resolve_selection, Drive};
    let prev = Drive {
        device: "/dev/sg1".into(),
        vendor: "HL-DT-ST".into(),
        product: "BU40N".into(),
        revision: "1.03".into(),
        ..Default::default()
    };
    let after_flash = vec![Drive {
        device: "/dev/sg3".into(),
        vendor: "HL-DT-ST".into(),
        product: "BU40N".into(),
        revision: "1.03".into(),
        ..Default::default()
    }];
    assert_eq!(
        resolve_selection(&after_flash, Some(&prev), Some(0)),
        Some(0)
    );
    assert_eq!(after_flash[0].build_drive_id(), "HL-DT-ST_BU40N_1.03");
}
