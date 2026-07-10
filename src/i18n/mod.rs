//! Localization: language detection, keys, and dispatch.
//!
//! English strings live in [`en`]; other locales in [`locales`].

mod en;
mod locales;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Language {
    Auto,
    English,
    Bulgarian,
    Croatian,
    Czech,
    Danish,
    Dutch,
    Estonian,
    Finnish,
    French,
    Galician,
    German,
    Greek,
    Hungarian,
    Indonesian,
    Italian,
    Latvian,
    Lithuanian,
    Malay,
    Norwegian,
    Polish,
    Portuguese,
    PortugueseBrazilian,
    Romanian,
    Russian,
    Slovak,
    Slovenian,
    Spanish,
    Swedish,
    Turkish,
    Ukrainian,
}

impl Language {
    pub const ALL: &[Language] = &[
        Language::Auto,
        Language::English,
        Language::Bulgarian,
        Language::Croatian,
        Language::Czech,
        Language::Danish,
        Language::Dutch,
        Language::Estonian,
        Language::Finnish,
        Language::French,
        Language::Galician,
        Language::German,
        Language::Greek,
        Language::Hungarian,
        Language::Indonesian,
        Language::Italian,
        Language::Latvian,
        Language::Lithuanian,
        Language::Malay,
        Language::Norwegian,
        Language::Polish,
        Language::Portuguese,
        Language::PortugueseBrazilian,
        Language::Romanian,
        Language::Russian,
        Language::Slovak,
        Language::Slovenian,
        Language::Spanish,
        Language::Swedish,
        Language::Turkish,
        Language::Ukrainian,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Auto => "Auto-detect",
            Self::English => "English (English)",
            Self::Bulgarian => "Български (Bulgarian)",
            Self::Croatian => "Hrvatski (Croatian)",
            Self::Czech => "Čeština (Czech)",
            Self::Danish => "Dansk (Danish)",
            Self::Dutch => "Nederlands (Dutch)",
            Self::Estonian => "Eesti (Estonian)",
            Self::Finnish => "Suomi (Finnish)",
            Self::French => "Français (French)",
            Self::Galician => "Galego (Galician)",
            Self::German => "Deutsch (German)",
            Self::Greek => "Ελληνικά (Greek)",
            Self::Hungarian => "Magyar (Hungarian)",
            Self::Indonesian => "Bahasa Indonesia (Indonesian)",
            Self::Italian => "Italiano (Italian)",
            Self::Latvian => "Latviešu (Latvian)",
            Self::Lithuanian => "Lietuvių (Lithuanian)",
            Self::Malay => "Bahasa Melayu (Malay)",
            Self::Norwegian => "Norsk (Norwegian)",
            Self::Polish => "Polski (Polish)",
            Self::Portuguese => "Português (Portuguese)",
            Self::PortugueseBrazilian => "Português do Brasil (Brazilian Portuguese)",
            Self::Romanian => "Română (Romanian)",
            Self::Russian => "Русский (Russian)",
            Self::Slovak => "Slovenčina (Slovak)",
            Self::Slovenian => "Slovenščina (Slovenian)",
            Self::Spanish => "Español (Spanish)",
            Self::Swedish => "Svenska (Swedish)",
            Self::Turkish => "Türkçe (Turkish)",
            Self::Ukrainian => "Українська (Ukrainian)",
        }
    }
}

/// Map a BCP-47 locale string (case-insensitive) to a supported language.
/// Returns `English` for any unrecognized prefix.
pub fn locale_to_language(locale: &str) -> Language {
    let locale = locale.to_lowercase();
    if locale.starts_with("bg") {
        Language::Bulgarian
    } else if locale.starts_with("hr") {
        Language::Croatian
    } else if locale.starts_with("cs") {
        Language::Czech
    } else if locale.starts_with("da") {
        Language::Danish
    } else if locale.starts_with("nl") {
        Language::Dutch
    } else if locale.starts_with("et") {
        Language::Estonian
    } else if locale.starts_with("fi") {
        Language::Finnish
    } else if locale.starts_with("fr") {
        Language::French
    } else if locale.starts_with("gl") {
        Language::Galician
    } else if locale.starts_with("de") {
        Language::German
    } else if locale.starts_with("el") {
        Language::Greek
    } else if locale.starts_with("hu") {
        Language::Hungarian
    } else if locale.starts_with("id") {
        Language::Indonesian
    } else if locale.starts_with("it") {
        Language::Italian
    } else if locale.starts_with("lv") {
        Language::Latvian
    } else if locale.starts_with("lt") {
        Language::Lithuanian
    } else if locale.starts_with("ms") {
        Language::Malay
    } else if locale.starts_with("nb") || locale.starts_with("no") || locale.starts_with("nn") {
        Language::Norwegian
    } else if locale.starts_with("pl") {
        Language::Polish
    } else if locale.starts_with("pt-br") {
        Language::PortugueseBrazilian
    } else if locale.starts_with("pt") {
        Language::Portuguese
    } else if locale.starts_with("ro") {
        Language::Romanian
    } else if locale.starts_with("ru") {
        Language::Russian
    } else if locale.starts_with("sk") {
        Language::Slovak
    } else if locale.starts_with("sl") {
        Language::Slovenian
    } else if locale.starts_with("es") {
        Language::Spanish
    } else if locale.starts_with("sv") {
        Language::Swedish
    } else if locale.starts_with("tr") {
        Language::Turkish
    } else if locale.starts_with("uk") {
        Language::Ukrainian
    } else {
        Language::English
    }
}

pub(crate) fn detect_system_language_from_locale(locale: Option<&str>) -> Language {
    if let Some(locale) = locale {
        let primary = locale_to_language(locale);
        if primary != Language::English {
            return primary;
        }
        // Region/script fallback: e.g. pt-PT → pt, zh-Hant-TW → zh (when translations exist).
        let mut remainder = locale;
        while let Some((_, rest)) = remainder.rsplit_once(['-', '_']) {
            remainder = rest;
            let fallback = locale_to_language(remainder);
            if fallback != Language::English {
                return fallback;
            }
        }
        primary
    } else {
        Language::English
    }
}

pub fn detect_system_language() -> Language {
    detect_system_language_from_locale(sys_locale::get_locale().as_deref())
}

pub fn resolve_language(lang: Language) -> Language {
    if lang == Language::Auto {
        detect_system_language()
    } else {
        lang
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L10nKey {
    TitleDriveProperties,
    LabelDevice,
    SectionOperation,
    TabWrite,
    TabRead,
    TabRecover,
    OptionBootloader,
    OptionEncrypted,
    SectionFirmwareImage,
    BtnBrowse,
    SectionStatus,
    LabelTypeToConfirm,
    LabelWrongFw,
    BtnExtract,
    BtnStart,
    BtnStop,
    MenuQuit,
    TooltipStop,
    TitleStopWarning,
    LabelStopWarningMsg,
    LabelStopWarningDesc,
    LabelStopWarningAsk,
    BtnStopNo,
    BtnStopYes,
    TitleForceKillWarning,
    LabelForceKillMsg,
    LabelForceKillDesc,
    LabelForceKillAsk,
    BtnForceKillNo,
    BtnForceKillYes,
    StatusCancelling,
    StatusOpCancelled,
    LogOpCancelled,
    StatusReady,
    StatusNoDrives,
    StatusProbing,
    StatusProbeFailed,
    StatusOpSuccess,
    TooltipRefresh,
    TooltipSettings,
    TooltipAbout,
    TooltipStartEnabled,
    TitleExitWarning,
    LabelExitWarningMsg,
    LabelExitWarningDesc,
    LabelExitWarningAsk,
    BtnNoCancel,
    BtnYesForce,
    TitleSettings,
    LabelBackend,
    LabelToolPath,
    LabelSdfPath,
    BtnListDrives,
    BtnParseSdf,
    LabelAutodetected,
    LabelLanguage,
    AboutDescription,
    AboutBuiltWith,
    AboutAcknowledgementsTitle,
    AboutBackendAckText,
    AboutCreatorAckText,
    ReasonBusy,
    ReasonProbing,
    ReasonNoDrive,
    ReasonNotMt1959,
    ReasonNoBackend,
    ReasonNoFirmware,
    ReasonConflict,
    ReasonEnterToken,
    // gui/mod.rs — drive properties
    LabelManufacturer,
    LabelProduct,
    LabelRevision,
    LabelSerial,
    LabelFirmwareDate,
    LabelMt1959Platform,
    LabelEncryptedFirmware,
    LabelLibreDrive,
    LibreDriveEnabled,
    LibreDrivePossible,
    LibreDriveNotAvailable,
    LibreDriveUnknown,
    LabelSdfVersion,
    WarnCannotCombine,
    StatusReadyText,
    LogReady,
    StatusNoDrivesFound,
    StatusDrivesFound,
    LabelToken,
    WarnFirmwareLoadFailed,
    LabelAppName,
    LabelGithubRepo,
    LabelVersion,
    BackendSdftool,
    BackendMakeMkv,
    BtnAutoDetect,
    StatusNotFound,
    StatusPathValid,
    StatusOptional,
    // gui/ops.rs
    StatusHintRead,
    StatusHintWrite,
    StatusHintRecover,
    ReasonInvalidToolPath,
    ReasonInvalidSdfPath,
    StatusReadingFirmware,
    StatusWritingFirmware,
    StatusRecoveringDrive,
    DialogTitleWrongFirmware,
    // gui/workers.rs
    StatusOpFinished,
    StatusOpFailed,
    StatusListingDrives,
    StatusDriveListFailed,
    // gui/validation.rs
    ValPathEmpty,
    ValFileNotExist,
    ValPathNotFile,
    ValMustContainSdftool,
    ValMustContainMakemkv,
    ValExtMustBeBin,
    // flash.rs
    ThemeSystem,
    ThemeDark,
    ThemeLight,
    // gui log messages (GUI-generated only; backend stdout is shown as-is)
    LogErrGeneric,
    LogFirmwareEmpty,
    LogFirmwareReadFailed,
    LogFirmwareLoaded,
    LogRecoverSelectWrongFw,
    LogRecoveryTokenExtracted,
    LogProbeResult,
    LogParsedDrivesFromOutput,
    LogSdfHeader,
    LogSdfVendor,
    LogSdfModel,
    LogSdfFirmware,
    LogSdfFlags,
    LogSdfExtraField,
    LogSdfReadFailed,
    // flash confirmation summary & failure recovery
    LabelFlashSummaryTitle,
    LabelFlashSummaryDrive,
    LabelFlashSummaryFirmware,
    LabelFlashSummaryMode,
    FlashModeStandard,
    FlashModeEncrypted,
    FlashModeBootloader,
    FlashModeRecover,
    TitleFlashFailure,
    LabelFlashFailureMsg,
    LabelFlashFailureStep1,
    LabelFlashFailureStep2,
    LabelFlashFailureStep3,
    BtnFlashFailureDismiss,
    LabelNotAvailable,
    BannerNoBackend,
    LinkGetMakeMkv,
    OptionDryRunOnly,
    LogDryRunCommand,
    HintFlashNoCancel,
    HelpEmptyDrives,
    LabelTokenLength,
    // Platform safety warnings
    WarnPlatformMismatch,
    WarnCrossFlashConfirm,
    ReasonCrossFlashNotConfirmed,
    InfoTwoStepFlash,
    WarnFirmwareDowngrade,
    InfoFirmwareModelMismatch,
    ReasonMt1939NotCompatible,
}

pub fn t(key: L10nKey, lang: Language) -> &'static str {
    match lang {
        Language::Auto => en::t_en(key),
        Language::English => en::t_en(key),
        Language::Bulgarian => locales::t_bg(key),
        Language::Croatian => locales::t_hr(key),
        Language::Czech => locales::t_cs(key),
        Language::Danish => locales::t_da(key),
        Language::Dutch => locales::t_nl(key),
        Language::Estonian => locales::t_et(key),
        Language::Finnish => locales::t_fi(key),
        Language::French => locales::t_fr(key),
        Language::Galician => locales::t_gl(key),
        Language::German => locales::t_de(key),
        Language::Greek => locales::t_el(key),
        Language::Hungarian => locales::t_hu(key),
        Language::Indonesian => locales::t_id(key),
        Language::Italian => locales::t_it(key),
        Language::Latvian => locales::t_lv(key),
        Language::Lithuanian => locales::t_lt(key),
        Language::Malay => locales::t_ms(key),
        Language::Norwegian => locales::t_nb(key),
        Language::Polish => locales::t_pl(key),
        Language::Portuguese => locales::t_pt(key),
        Language::PortugueseBrazilian => locales::t_pt_br(key),
        Language::Romanian => locales::t_ro(key),
        Language::Russian => locales::t_ru(key),
        Language::Slovak => locales::t_sk(key),
        Language::Slovenian => locales::t_sl(key),
        Language::Spanish => locales::t_es(key),
        Language::Swedish => locales::t_sv(key),
        Language::Turkish => locales::t_tr(key),
        Language::Ukrainian => locales::t_uk(key),
    }
}

pub fn t_with_args(key: L10nKey, lang: Language, args: &[(&str, &str)]) -> String {
    let mut text = t(key, lang).to_string();
    for (k, v) in args {
        text = text.replace(&format!("{{{k}}}"), v);
    }
    text
}

/// Prefix a message with the localized ERROR label (for GUI-generated log lines).
pub fn log_error(lang: Language, message: &str) -> String {
    t_with_args(L10nKey::LogErrGeneric, lang, &[("message", message)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_display_names() {
        for lang in Language::ALL {
            let name = lang.display_name();
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn test_locale_region_fallback_portuguese() {
        assert_eq!(locale_to_language("pt-PT"), Language::Portuguese);
        assert_eq!(
            detect_system_language_from_locale(Some("pt-PT")),
            Language::Portuguese
        );
    }

    #[test]
    fn test_locale_region_fallback_from_unknown_primary() {
        // Primary tag is unknown; suffix "pt" should resolve via region fallback.
        assert_eq!(
            detect_system_language_from_locale(Some("xx-pt")),
            Language::Portuguese
        );
        // Underscore separator + German suffix (covers rsplit fallback return).
        assert_eq!(
            detect_system_language_from_locale(Some("zz_de")),
            Language::German
        );
        // Unknown primary and region → English primary after exhausted fallbacks.
        assert_eq!(
            detect_system_language_from_locale(Some("zz-yy")),
            Language::English
        );
    }

    #[test]
    fn test_detect_system_language_no_locale() {
        assert_eq!(detect_system_language_from_locale(None), Language::English);
    }

    #[test]
    fn test_resolve_language() {
        // Test that resolving a specific language returns itself
        assert_eq!(resolve_language(Language::French), Language::French);
        assert_eq!(resolve_language(Language::Spanish), Language::Spanish);

        // Test resolving Auto
        let resolved = resolve_language(Language::Auto);
        // It should match whatever detect_system_language() returns
        assert_eq!(resolved, detect_system_language());
    }

    const ALL_KEYS: &[L10nKey] = &[
        L10nKey::TitleDriveProperties,
        L10nKey::LabelDevice,
        L10nKey::SectionOperation,
        L10nKey::TabWrite,
        L10nKey::TabRead,
        L10nKey::TabRecover,
        L10nKey::OptionBootloader,
        L10nKey::OptionEncrypted,
        L10nKey::SectionFirmwareImage,
        L10nKey::BtnBrowse,
        L10nKey::SectionStatus,
        L10nKey::LabelTypeToConfirm,
        L10nKey::LabelWrongFw,
        L10nKey::BtnExtract,
        L10nKey::BtnStart,
        L10nKey::BtnStop,
        L10nKey::MenuQuit,
        L10nKey::TooltipStop,
        L10nKey::TitleStopWarning,
        L10nKey::LabelStopWarningMsg,
        L10nKey::LabelStopWarningDesc,
        L10nKey::LabelStopWarningAsk,
        L10nKey::BtnStopNo,
        L10nKey::BtnStopYes,
        L10nKey::TitleForceKillWarning,
        L10nKey::LabelForceKillMsg,
        L10nKey::LabelForceKillDesc,
        L10nKey::LabelForceKillAsk,
        L10nKey::BtnForceKillNo,
        L10nKey::BtnForceKillYes,
        L10nKey::StatusCancelling,
        L10nKey::StatusOpCancelled,
        L10nKey::LogOpCancelled,
        L10nKey::StatusReady,
        L10nKey::StatusNoDrives,
        L10nKey::StatusProbing,
        L10nKey::StatusProbeFailed,
        L10nKey::StatusOpSuccess,
        L10nKey::TooltipRefresh,
        L10nKey::TooltipSettings,
        L10nKey::TooltipAbout,
        L10nKey::TooltipStartEnabled,
        L10nKey::TitleExitWarning,
        L10nKey::LabelExitWarningMsg,
        L10nKey::LabelExitWarningDesc,
        L10nKey::LabelExitWarningAsk,
        L10nKey::BtnNoCancel,
        L10nKey::BtnYesForce,
        L10nKey::TitleSettings,
        L10nKey::LabelBackend,
        L10nKey::LabelToolPath,
        L10nKey::LabelSdfPath,
        L10nKey::BtnListDrives,
        L10nKey::BtnParseSdf,
        L10nKey::LabelAutodetected,
        L10nKey::LabelLanguage,
        L10nKey::AboutDescription,
        L10nKey::AboutBuiltWith,
        L10nKey::AboutAcknowledgementsTitle,
        L10nKey::AboutBackendAckText,
        L10nKey::AboutCreatorAckText,
        L10nKey::ReasonBusy,
        L10nKey::ReasonProbing,
        L10nKey::ReasonNoDrive,
        L10nKey::ReasonNotMt1959,
        L10nKey::ReasonNoBackend,
        L10nKey::ReasonNoFirmware,
        L10nKey::ReasonConflict,
        L10nKey::ReasonEnterToken,
        L10nKey::LabelManufacturer,
        L10nKey::LabelProduct,
        L10nKey::LabelRevision,
        L10nKey::LabelSerial,
        L10nKey::LabelFirmwareDate,
        L10nKey::LabelMt1959Platform,
        L10nKey::LabelEncryptedFirmware,
        L10nKey::LabelLibreDrive,
        L10nKey::LibreDriveEnabled,
        L10nKey::LibreDrivePossible,
        L10nKey::LibreDriveNotAvailable,
        L10nKey::LibreDriveUnknown,
        L10nKey::LabelSdfVersion,
        L10nKey::WarnCannotCombine,
        L10nKey::StatusReadyText,
        L10nKey::LogReady,
        L10nKey::StatusNoDrivesFound,
        L10nKey::StatusDrivesFound,
        L10nKey::LabelToken,
        L10nKey::WarnFirmwareLoadFailed,
        L10nKey::LabelAppName,
        L10nKey::LabelGithubRepo,
        L10nKey::LabelVersion,
        L10nKey::BackendSdftool,
        L10nKey::BackendMakeMkv,
        L10nKey::BtnAutoDetect,
        L10nKey::StatusNotFound,
        L10nKey::StatusPathValid,
        L10nKey::StatusOptional,
        L10nKey::StatusHintRead,
        L10nKey::StatusHintWrite,
        L10nKey::StatusHintRecover,
        L10nKey::ReasonInvalidToolPath,
        L10nKey::ReasonInvalidSdfPath,
        L10nKey::StatusReadingFirmware,
        L10nKey::StatusWritingFirmware,
        L10nKey::StatusRecoveringDrive,
        L10nKey::DialogTitleWrongFirmware,
        L10nKey::StatusOpFinished,
        L10nKey::StatusOpFailed,
        L10nKey::StatusListingDrives,
        L10nKey::StatusDriveListFailed,
        L10nKey::ValPathEmpty,
        L10nKey::ValFileNotExist,
        L10nKey::ValPathNotFile,
        L10nKey::ValMustContainSdftool,
        L10nKey::ValMustContainMakemkv,
        L10nKey::ValExtMustBeBin,
        L10nKey::ThemeSystem,
        L10nKey::ThemeDark,
        L10nKey::ThemeLight,
        L10nKey::LogErrGeneric,
        L10nKey::LogFirmwareEmpty,
        L10nKey::LogFirmwareReadFailed,
        L10nKey::LogFirmwareLoaded,
        L10nKey::LogRecoverSelectWrongFw,
        L10nKey::LogRecoveryTokenExtracted,
        L10nKey::LogProbeResult,
        L10nKey::LogParsedDrivesFromOutput,
        L10nKey::LogSdfHeader,
        L10nKey::LogSdfVendor,
        L10nKey::LogSdfModel,
        L10nKey::LogSdfFirmware,
        L10nKey::LogSdfFlags,
        L10nKey::LogSdfExtraField,
        L10nKey::LogSdfReadFailed,
        L10nKey::LabelFlashSummaryTitle,
        L10nKey::LabelFlashSummaryDrive,
        L10nKey::LabelFlashSummaryFirmware,
        L10nKey::LabelFlashSummaryMode,
        L10nKey::FlashModeStandard,
        L10nKey::FlashModeEncrypted,
        L10nKey::FlashModeBootloader,
        L10nKey::FlashModeRecover,
        L10nKey::TitleFlashFailure,
        L10nKey::LabelFlashFailureMsg,
        L10nKey::LabelFlashFailureStep1,
        L10nKey::LabelFlashFailureStep2,
        L10nKey::LabelFlashFailureStep3,
        L10nKey::BtnFlashFailureDismiss,
        L10nKey::LabelNotAvailable,
        L10nKey::BannerNoBackend,
        L10nKey::LinkGetMakeMkv,
        L10nKey::OptionDryRunOnly,
        L10nKey::LogDryRunCommand,
        L10nKey::HintFlashNoCancel,
        L10nKey::HelpEmptyDrives,
        L10nKey::LabelTokenLength,
        L10nKey::WarnPlatformMismatch,
        L10nKey::WarnCrossFlashConfirm,
        L10nKey::ReasonCrossFlashNotConfirmed,
        L10nKey::InfoTwoStepFlash,
        L10nKey::WarnFirmwareDowngrade,
        L10nKey::InfoFirmwareModelMismatch,
        L10nKey::ReasonMt1939NotCompatible,
    ];

    #[test]
    fn test_all_keys_non_empty_for_each_language() {
        for &lang in Language::ALL {
            if lang == Language::Auto {
                continue;
            }
            for key in ALL_KEYS {
                assert!(!t(*key, lang).is_empty(), "{lang:?} missing {key:?}");
            }
        }
    }

    #[test]
    fn test_translation_keys() {
        assert_eq!(
            ALL_KEYS.len(),
            164,
            "L10nKey variant count changed — update ALL_KEYS if intentional"
        );
        for key in ALL_KEYS {
            let translation = t(*key, Language::English);
            assert!(
                !translation.is_empty(),
                "empty translation for key: {key:?}"
            );
        }
    }

    #[test]
    fn test_translation_with_args() {
        let args = [("required", "WRITE")];
        let translation = t_with_args(L10nKey::LabelTypeToConfirm, Language::English, &args);
        assert_eq!(translation, "Type \"WRITE\" to confirm:");
    }

    #[test]
    fn test_platform_safety_keys_in_all_keys() {
        let new_keys = [
            L10nKey::WarnPlatformMismatch,
            L10nKey::WarnCrossFlashConfirm,
            L10nKey::ReasonCrossFlashNotConfirmed,
            L10nKey::InfoTwoStepFlash,
            L10nKey::WarnFirmwareDowngrade,
            L10nKey::InfoFirmwareModelMismatch,
            L10nKey::ReasonMt1939NotCompatible,
        ];
        for &key in &new_keys {
            assert!(
                ALL_KEYS.contains(&key),
                "new safety key {key:?} missing from ALL_KEYS"
            );
        }
    }

    #[test]
    fn test_platform_safety_keys_non_empty_english() {
        let keys = [
            L10nKey::WarnPlatformMismatch,
            L10nKey::WarnCrossFlashConfirm,
            L10nKey::ReasonCrossFlashNotConfirmed,
            L10nKey::InfoTwoStepFlash,
            L10nKey::WarnFirmwareDowngrade,
            L10nKey::InfoFirmwareModelMismatch,
            L10nKey::ReasonMt1939NotCompatible,
        ];
        for &key in &keys {
            let text = t(key, Language::English);
            assert!(!text.is_empty(), "empty English text for {key:?}");
        }
    }

    #[test]
    fn test_locale_to_language_all_prefixes() {
        let cases = [
            ("bg", Language::Bulgarian),
            ("bg-BG", Language::Bulgarian),
            ("hr", Language::Croatian),
            ("hr-HR", Language::Croatian),
            ("cs", Language::Czech),
            ("cs-CZ", Language::Czech),
            ("da", Language::Danish),
            ("da-DK", Language::Danish),
            ("nl", Language::Dutch),
            ("nl-NL", Language::Dutch),
            ("et", Language::Estonian),
            ("et-EE", Language::Estonian),
            ("fi", Language::Finnish),
            ("fi-FI", Language::Finnish),
            ("fr", Language::French),
            ("fr-FR", Language::French),
            ("gl", Language::Galician),
            ("gl-ES", Language::Galician),
            ("de", Language::German),
            ("de-DE", Language::German),
            ("el", Language::Greek),
            ("el-GR", Language::Greek),
            ("hu", Language::Hungarian),
            ("hu-HU", Language::Hungarian),
            ("id", Language::Indonesian),
            ("id-ID", Language::Indonesian),
            ("it", Language::Italian),
            ("it-IT", Language::Italian),
            ("lv", Language::Latvian),
            ("lv-LV", Language::Latvian),
            ("lt", Language::Lithuanian),
            ("lt-LT", Language::Lithuanian),
            ("ms", Language::Malay),
            ("ms-MY", Language::Malay),
            ("nb", Language::Norwegian),
            ("nb-NO", Language::Norwegian),
            ("no", Language::Norwegian),
            ("nn", Language::Norwegian),
            ("pl", Language::Polish),
            ("pl-PL", Language::Polish),
            ("pt-br", Language::PortugueseBrazilian),
            ("pt-BR", Language::PortugueseBrazilian),
            ("pt", Language::Portuguese),
            ("pt-PT", Language::Portuguese),
            ("ro", Language::Romanian),
            ("ro-RO", Language::Romanian),
            ("ru", Language::Russian),
            ("ru-RU", Language::Russian),
            ("sk", Language::Slovak),
            ("sk-SK", Language::Slovak),
            ("sl", Language::Slovenian),
            ("sl-SI", Language::Slovenian),
            ("es", Language::Spanish),
            ("es-ES", Language::Spanish),
            ("sv", Language::Swedish),
            ("sv-SE", Language::Swedish),
            ("tr", Language::Turkish),
            ("tr-TR", Language::Turkish),
            ("uk", Language::Ukrainian),
            ("uk-UA", Language::Ukrainian),
            ("en", Language::English),
            ("en-US", Language::English),
            ("ja", Language::English), // unsupported → English
            ("zh-CN", Language::English),
        ];
        for (locale, expected) in &cases {
            assert_eq!(
                locale_to_language(locale),
                *expected,
                "locale_to_language({locale}) should return {expected:?}"
            );
        }
    }

    #[test]
    fn test_all_display_names_unique() {
        let mut names = std::collections::HashSet::new();
        for lang in Language::ALL {
            let name = lang.display_name();
            assert!(names.insert(name), "duplicate display name: {name}");
        }
    }

    #[test]
    fn test_display_name_japanese_not_present() {
        // Japanese is not in the supported list
        assert!(!Language::ALL
            .iter()
            .any(|l| l.display_name().contains("Japanese")));
    }

    #[test]
    fn test_resolve_language_specific() {
        for lang in Language::ALL {
            if *lang == Language::Auto {
                continue;
            }
            assert_eq!(resolve_language(*lang), *lang);
        }
    }

    #[test]
    fn test_t_with_args_no_args() {
        let text = t_with_args(L10nKey::TitleDriveProperties, Language::English, &[]);
        assert_eq!(text, "Drive Properties");
    }

    #[test]
    fn test_t_with_args_multiple_args() {
        // LabelTypeToConfirm has {required} placeholder
        let args = [("required", "FLASH H:")];
        let text = t_with_args(L10nKey::LabelTypeToConfirm, Language::English, &args);
        assert!(text.contains("FLASH H:"));
    }

    #[test]
    fn test_t_with_args_missing_placeholder() {
        // If placeholder doesn't exist, text should be unchanged
        let args = [("nonexistent", "value")];
        let text = t_with_args(L10nKey::TitleDriveProperties, Language::English, &args);
        assert_eq!(text, "Drive Properties");
    }

    #[test]
    fn test_language_all_count() {
        // 31 languages including Auto
        assert_eq!(Language::ALL.len(), 31);
    }

    #[test]
    fn test_translation_dispatch_returns_language_specific_text() {
        // German has real translations — should differ from English
        let en = t(L10nKey::TitleSettings, Language::English);
        let de = t(L10nKey::TitleSettings, Language::German);
        assert_eq!(en, "Settings");
        assert_eq!(de, "Einstellungen");
        assert_ne!(en, de, "German translation should differ from English");
    }

    #[test]
    fn test_auto_language_falls_back_to_english() {
        // Language::Auto should return English (resolve happens outside t())
        let en = t(L10nKey::TitleSettings, Language::English);
        let auto = t(L10nKey::TitleSettings, Language::Auto);
        assert_eq!(en, auto, "Language::Auto should fall back to English");
    }

    #[test]
    fn test_german_translations_complete() {
        // German has full translations — spot-check keys where the text must differ
        let must_differ = [
            L10nKey::TitleSettings,
            L10nKey::LabelToolPath,
            L10nKey::BtnListDrives,
            L10nKey::BtnParseSdf,
            L10nKey::LabelAutodetected,
            L10nKey::LabelLanguage,
            L10nKey::BtnStart,
            L10nKey::StatusReady,
            L10nKey::TooltipSettings,
        ];
        for key in must_differ {
            let text = t(key, Language::German);
            assert!(!text.is_empty(), "German translation missing for {key:?}");
            assert_ne!(
                text,
                t(key, Language::English),
                "German should have a real translation for {key:?}, not English fallback"
            );
        }
        // Keys like "Backend:", "sdf.bin:" are the same in German — just verify non-empty
        let same_in_both = [L10nKey::LabelBackend, L10nKey::LabelSdfPath];
        for key in same_in_both {
            let text = t(key, Language::German);
            assert!(!text.is_empty(), "German translation missing for {key:?}");
        }
    }

    /// GUI log/error keys added for ops/workers — must be defined in every locale table.
    const LOG_KEYS: &[L10nKey] = &[
        L10nKey::LogErrGeneric,
        L10nKey::LogFirmwareEmpty,
        L10nKey::LogFirmwareReadFailed,
        L10nKey::LogFirmwareLoaded,
        L10nKey::LogRecoverSelectWrongFw,
        L10nKey::LogRecoveryTokenExtracted,
        L10nKey::LogProbeResult,
        L10nKey::LogParsedDrivesFromOutput,
        L10nKey::LogSdfHeader,
        L10nKey::LogSdfVendor,
        L10nKey::LogSdfModel,
        L10nKey::LogSdfFirmware,
        L10nKey::LogSdfFlags,
        L10nKey::LogSdfExtraField,
        L10nKey::LogSdfReadFailed,
    ];

    #[test]
    fn test_log_error_helper() {
        let msg = log_error(Language::Spanish, "boom");
        assert!(msg.contains("boom"));
    }

    #[test]
    fn test_log_keys_translated_all_languages() {
        for lang in Language::ALL {
            if matches!(lang, Language::Auto | Language::English) {
                continue;
            }
            let mut localized = 0usize;
            for key in LOG_KEYS {
                let text = t(*key, *lang);
                assert!(!text.is_empty(), "{lang:?} missing translation for {key:?}");
                if text != t(*key, Language::English) {
                    localized += 1;
                }
            }
            // Allow a few technical strings to match English (e.g. SDF0 header, ERROR prefix).
            let min_localized = LOG_KEYS.len() - 6;
            assert!(
                localized >= min_localized,
                "{lang:?} has too many English fallbacks for log keys ({localized}/{min_localized})"
            );
        }
    }
}
