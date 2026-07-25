use super::L10nKey;

pub(super) fn t_en(key: L10nKey) -> &'static str {
    match key {
        L10nKey::TitleDriveProperties => "Drive Properties",
        L10nKey::LabelDevice => "Device",
        L10nKey::SectionOperation => "Operation",
        L10nKey::TabWrite => "Write Firmware",
        L10nKey::TabRead => "Read Firmware",
        L10nKey::TabRecover => "Recover Drive",
        L10nKey::OptionBootloader => "Include boot-loader (dangerous)",
        L10nKey::OptionEncrypted => "Encrypted rawflash",
        L10nKey::SectionFirmwareImage => "Firmware Image",
        L10nKey::BtnBrowse => "Browse…",
        L10nKey::SectionStatus => "Status",
        L10nKey::LabelTypeToConfirm => "Type \"{required}\" to confirm:",
        L10nKey::LabelWrongFw => "Wrong firmware",
        L10nKey::BtnExtract => "Extract",
        L10nKey::BtnStart => "START",
        L10nKey::BtnStop => "STOP",
        L10nKey::MenuQuit => "Quit",
        L10nKey::TooltipStop => "Stop the current operation",
        L10nKey::TitleStopWarning => "Confirm Stop",
        L10nKey::LabelStopWarningMsg => "An operation is in progress!",
        L10nKey::LabelStopWarningDesc => "Stopping now may interrupt the flashing process and brick your optical drive.",
        L10nKey::LabelStopWarningAsk => "Are you sure you want to stop this operation?",
        L10nKey::BtnStopNo => "No, Keep Running",
        L10nKey::BtnStopYes => "Yes, Stop",
        L10nKey::TitleForceKillWarning => "Force Stop",
        L10nKey::LabelForceKillMsg => "The backend did not stop!",
        L10nKey::LabelForceKillDesc => "Force-killing the backend may corrupt the drive. Only continue if the operation appears hung.",
        L10nKey::LabelForceKillAsk => "Force kill the backend process?",
        L10nKey::BtnForceKillNo => "No, Wait",
        L10nKey::BtnForceKillYes => "Yes, Force Kill",
        L10nKey::StatusCancelling => "Cancelling…",
        L10nKey::StatusOpCancelled => "Operation cancelled",
        L10nKey::LogOpCancelled => "Operation cancelled by user.",
        L10nKey::StatusReady => "Ready",
        L10nKey::StatusNoDrives => "No optical drives detected",
        L10nKey::StatusProbing => "Probing drive",
        L10nKey::StatusProbeFailed => "Drive probe failed",
        L10nKey::StatusOpSuccess => "Operation completed successfully.",
        L10nKey::TooltipRefresh => "Refresh drives",
        L10nKey::TooltipSettings => "Settings",
        L10nKey::TooltipAbout => "About",
        L10nKey::TooltipStartEnabled => "Start the selected operation",
        L10nKey::TitleExitWarning => "Confirm Exit",
        L10nKey::LabelExitWarningMsg => "An operation is in progress!",
        L10nKey::LabelExitWarningDesc => "Closing the application now may interrupt the flashing process and brick your optical drive.",
        L10nKey::LabelExitWarningAsk => "Are you sure you want to force exit?",
        L10nKey::BtnNoCancel => "No, Cancel",
        L10nKey::BtnYesForce => "Yes, Force Exit",
        L10nKey::TitleSettings => "Settings",
        L10nKey::LabelBackend => "Backend:",
        L10nKey::LabelToolPath => "Tool path:",
        L10nKey::LabelSdfPath => "sdf.bin:",
        L10nKey::BtnListDrives => "List drives via backend",
        L10nKey::BtnParseSdf => "Parse sdf.bin",
        L10nKey::LabelAutodetected => "(auto-detected)",
        L10nKey::LabelLanguage => "Language:",
        L10nKey::AboutDescription => "A cross-platform GUI for flashing optical drives.",
        L10nKey::AboutBuiltWith => "Built with Rust and egui.",
        L10nKey::AboutAcknowledgementsTitle => "Acknowledgements:",
        L10nKey::AboutBackendAckText => "for providing the sdftool/makemkvcon backend.",
        L10nKey::AboutCreatorAckText => "for creating the original SDFtool Flasher.",
        L10nKey::ReasonBusy => "Operation in progress",
        L10nKey::ReasonProbing => "Probing drive",
        L10nKey::ReasonNoDrive => "Select a drive first",
        L10nKey::ReasonNotMt1959 => "Drive is not MT1959 platform",
        L10nKey::ReasonNoBackend => "Configure backend in Settings",
        L10nKey::ReasonNoFirmware => "Select a firmware file",
        L10nKey::ReasonConflict => "Encrypted and boot-loader modes conflict",
        L10nKey::ReasonEnterToken => "Enter recovery token and confirmation",
        L10nKey::LabelManufacturer => "Manufacturer:",
        L10nKey::LabelProduct => "Product:",
        L10nKey::LabelRevision => "Revision:",
        L10nKey::LabelSerial => "Serial:",
        L10nKey::LabelFirmwareDate => "Firmware date:",
        L10nKey::LabelMt1959Platform => "MT1959 Platform:",
        L10nKey::LabelEncryptedFirmware => "Encrypted Firmware:",
        L10nKey::LabelLibreDrive => "LibreDrive:",
        L10nKey::LibreDriveEnabled => "Enabled",
        L10nKey::LibreDrivePossible => "Possible, not yet enabled",
        L10nKey::LibreDriveNotAvailable => "Not available",
        L10nKey::LibreDriveUnknown => "Unknown",
        L10nKey::LabelSdfVersion => "SDF Version:",
        L10nKey::WarnCannotCombine => "Cannot combine encrypted + boot-loader",
        L10nKey::StatusReadyText => "READY",
        L10nKey::LogReady => "Ready.",
        L10nKey::StatusDrivesFound => "{count} drives found",
        L10nKey::StatusOneDriveFound => "1 drive found",
        L10nKey::LabelToken => "Token:",
        L10nKey::WarnFirmwareLoadFailed => "Failed to load or invalid firmware file",
        L10nKey::LabelAppName => crate::branding::DISPLAY_NAME,
        L10nKey::LabelGithubRepo => "GitHub Repository",
        L10nKey::LabelVersion => "Version {version}",
        L10nKey::BackendSdftool => "SDFtool",
        L10nKey::BackendMakeMkv => "MakeMKV (makemkvcon)",
        L10nKey::BtnAutoDetect => "Auto-detect",
        L10nKey::StatusNotFound => "✗ Not found",
        L10nKey::StatusPathValid => "✓ Path is valid",
        L10nKey::StatusOptional => "Optional",
        L10nKey::StatusHintRead => "Select output folder when you start",
        L10nKey::StatusHintWrite => "Load firmware, then confirm",
        L10nKey::StatusHintRecover => "Recovery needs boot token from wrong firmware",
        L10nKey::ReasonInvalidToolPath => "Invalid tool path: {error}",
        L10nKey::ReasonInvalidSdfPath => "Invalid sdf.bin: {error}",
        L10nKey::StatusReadingFirmware => "Reading firmware",
        L10nKey::StatusWritingFirmware => "Writing firmware",
        L10nKey::StatusRecoveringDrive => "Recovering drive",
        L10nKey::DialogTitleWrongFirmware => "Wrong firmware (for token extraction)",
        L10nKey::StatusOpFinished => "Operation finished — please wait…",
        L10nKey::StatusOpFailed => "Operation failed",
        L10nKey::StatusListingDrives => "Listing drives",
        L10nKey::StatusDriveListFailed => "Drive list failed",
        L10nKey::ValPathEmpty => "Path is empty",
        L10nKey::ValFileNotExist => "File does not exist",
        L10nKey::ValPathNotFile => "Path is not a file",
        L10nKey::ValMustContainSdftool => "Filename must contain 'sdftool'",
        L10nKey::ValMustContainMakemkv => "Filename must contain 'makemkvcon' or 'makemkv'",
        L10nKey::ValExtMustBeBin => "File extension must be '.bin'",
        L10nKey::ThemeSystem => "System",
        L10nKey::ThemeDark => "Dark",
        L10nKey::ThemeLight => "Light",
        L10nKey::LogErrGeneric => "ERROR: {message}",
        L10nKey::LogFirmwareEmpty => "ERROR: firmware file is empty: {path}",
        L10nKey::LogFirmwareReadFailed => "ERROR: cannot read firmware file {path}: {error}",
        L10nKey::LogFirmwareLoaded => {
            "Loaded firmware: {path} ({size} bytes, sha256 {hash})"
        }
        L10nKey::LogRecoverSelectWrongFw => {
            "Recover: select the wrong firmware file to extract boot token"
        }
        L10nKey::LogRecoveryTokenExtracted => "Extracted recovery boot token: {token}",
        L10nKey::LogProbeResult => "MT1959: {mt1959} | Encrypted FW: {encrypted}",
        L10nKey::LogParsedDrivesFromOutput => "Parsed {count} drives from output.",
        L10nKey::LogParsedOneDriveFromOutput => "Parsed 1 drive from output.",
        L10nKey::LogSdfHeader => {
            "SDF0 v{version} | header_size={header_size} | payload_offset={offset}"
        }
        L10nKey::LogSdfVendor => "  Vendor: {vendor}",
        L10nKey::LogSdfModel => "  Model: {model}",
        L10nKey::LogSdfFirmware => "  Firmware: {firmware}",
        L10nKey::LogSdfFlags => "  Encrypted: {encrypted} | Compressed: {compressed}",
        L10nKey::LogSdfExtraField => "  {key}: {value}",
        L10nKey::LogSdfReadFailed => "ERROR: cannot read sdf.bin: {error}",        L10nKey::LabelFlashSummaryTitle => "Operation summary",
        L10nKey::LabelFlashSummaryDrive => "Drive: {label} ({device})",
        L10nKey::LabelFlashSummaryFirmware => "Firmware: {file} (SHA-256 {hash}…)",
        L10nKey::LabelFlashSummaryMode => "Mode: {mode}",
        L10nKey::FlashModeStandard => "Standard write",
        L10nKey::FlashModeEncrypted => "Encrypted rawflash",
        L10nKey::FlashModeBootloader => "Boot-loader rawflash (dangerous)",
        L10nKey::FlashModeRecover => "Recovery flash",
        L10nKey::TitleFlashFailure => "Flash Operation Failed",
        L10nKey::LabelFlashFailureMsg => {
            "The firmware write may have left your drive in an inconsistent state."
        }
        L10nKey::LabelFlashFailureStep1 => "Do not power off or eject the drive.",
        L10nKey::LabelFlashFailureStep2 => "Check the log below for error details.",
        L10nKey::LabelFlashFailureStep3 => {
            "If the drive is unresponsive, switch to Recover Drive mode and use a recovery token."
        }
        L10nKey::BtnFlashFailureDismiss => "I Understand",
        L10nKey::LabelNotAvailable => "N/A",
        L10nKey::BannerNoBackend => "No backend tool configured.",
        L10nKey::LinkGetMakeMkv => "Get MakeMKV",
        L10nKey::OptionDryRunOnly => "Dry-run only (no write)",
        L10nKey::LogDryRunCommand => "Dry-run — command that would run:\n{command}",
        L10nKey::HintFlashNoCancel => "Flash in progress — do not power off. Stop may brick the drive.",
        L10nKey::HelpEmptyDrives => {
            #[cfg(target_os = "linux")]
            {
                "Check the connection, permissions (cdrom group), and refresh."
            }
            #[cfg(target_os = "macos")]
            {
                "Check the connection, power/USB, and refresh."
            }
            #[cfg(target_os = "windows")]
            {
                "Check the connection and refresh."
            }
            #[cfg(not(any(
                target_os = "linux",
                target_os = "macos",
                target_os = "windows"
            )))]
            {
                "Check the connection and refresh."
            }
        }
        L10nKey::LabelTokenLength => "{current}/16",
        L10nKey::WarnPlatformMismatch => "WARNING: This firmware is for {firmware} drives but your drive is {drive}. Flashing wrong form factor firmware can BRICK your drive.",
        L10nKey::WarnCrossFlashConfirm => "I understand this is a cross-flash and want to proceed",
        L10nKey::ReasonCrossFlashNotConfirmed => "Confirm cross-flash to proceed",
        L10nKey::InfoTwoStepFlash => "This drive model (BP50NB40/WP50NB40/BP55EB40) requires two-step flashing. Step 1: Flash the intermediate MK firmware (e.g. DE_LG_BP50NB40-NB50_1.03_MK.bin) in Write mode. Step 2: Switch to Recover mode and flash your target firmware.",
        L10nKey::WarnFirmwareDowngrade => "This is a firmware downgrade (current: {current}, target: {target}). If downgrading from encrypted firmware, ensure 'Encrypted' is checked.",
        L10nKey::InfoFirmwareModelMismatch => "Firmware is for {firmware}, your drive is {drive}. This is normal for cross-flashing.",
        L10nKey::ReasonMt1939NotCompatible => "This drive uses the older MT1939 chip and is NOT compatible with OmniDrive or MK firmware. See: https://wiki.redump.info/index.php?title=Flashing_Older_HLDS_Drives",
        L10nKey::LogTruncated => "… (older log entries truncated)",
        L10nKey::StatusYes => "Yes",
        L10nKey::StatusNo => "No",
        L10nKey::DialogFilterFirmware => "Firmware",
        L10nKey::DialogFilterExecutable => "Executable",
    }
}
