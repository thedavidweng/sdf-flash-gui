//! Localization: language detection, keys, and dispatch.
//!
//! English strings live in [`en`]; other locales in [`locales`].

mod en;
mod locales;

macro_rules! languages {
    ($($lang:ident),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        pub enum Language {
            $($lang,)*
        }

        impl Language {
            pub const ALL: &'static [Language] = &[$(Language::$lang,)*];
        }
    };
}

languages! {
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

const LOCALE_PREFIXES: &[(&str, Language)] = &[
    ("bg", Language::Bulgarian),
    ("hr", Language::Croatian),
    ("cs", Language::Czech),
    ("da", Language::Danish),
    ("nl", Language::Dutch),
    ("et", Language::Estonian),
    ("fi", Language::Finnish),
    ("fr", Language::French),
    ("gl", Language::Galician),
    ("de", Language::German),
    ("el", Language::Greek),
    ("hu", Language::Hungarian),
    ("id", Language::Indonesian),
    ("it", Language::Italian),
    ("lv", Language::Latvian),
    ("lt", Language::Lithuanian),
    ("ms", Language::Malay),
    ("nb", Language::Norwegian),
    ("no", Language::Norwegian),
    ("nn", Language::Norwegian),
    ("pl", Language::Polish),
    ("pt-br", Language::PortugueseBrazilian),
    ("pt", Language::Portuguese),
    ("ro", Language::Romanian),
    ("ru", Language::Russian),
    ("sk", Language::Slovak),
    ("sl", Language::Slovenian),
    ("es", Language::Spanish),
    ("sv", Language::Swedish),
    ("tr", Language::Turkish),
    ("uk", Language::Ukrainian),
];

/// Map a BCP-47 locale string (case-insensitive) to a supported language.
/// Returns `English` for any unrecognized prefix.
pub fn locale_to_language(locale: &str) -> Language {
    let locale = locale.to_lowercase();
    LOCALE_PREFIXES
        .iter()
        .find(|(prefix, _)| locale.starts_with(prefix))
        .map_or(Language::English, |&(_, language)| language)
}

pub(crate) fn detect_system_language_from_locale(locale: Option<&str>) -> Language {
    if let Some(locale) = locale {
        let primary = locale_to_language(locale);
        if primary != Language::English {
            return primary;
        }
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

macro_rules! l10n_keys {
    ($($key:ident),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum L10nKey {
            $($key,)*
        }

        impl L10nKey {
            pub const ALL: &'static [L10nKey] = &[$(L10nKey::$key,)*];
        }
    };
}

l10n_keys! {
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
    StatusDrivesFound,
    StatusOneDriveFound,
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
    StatusHintRead,
    StatusHintWrite,
    StatusHintRecover,
    ReasonInvalidToolPath,
    ReasonInvalidSdfPath,
    StatusReadingFirmware,
    StatusWritingFirmware,
    StatusRecoveringDrive,
    DialogTitleWrongFirmware,
    StatusOpFinished,
    StatusOpFailed,
    StatusListingDrives,
    StatusDriveListFailed,
    ValPathEmpty,
    ValFileNotExist,
    ValPathNotFile,
    ValMustContainSdftool,
    ValMustContainMakemkv,
    ValExtMustBeBin,
    ThemeSystem,
    ThemeDark,
    ThemeLight,
    LogErrGeneric,
    LogFirmwareEmpty,
    LogFirmwareReadFailed,
    LogFirmwareTooLarge,
    LogFirmwareLoaded,
    LogRecoverSelectWrongFw,
    LogRecoveryTokenExtracted,
    LogProbeResult,
    LogParsedDrivesFromOutput,
    LogParsedOneDriveFromOutput,
    LogSdfHeader,
    LogSdfVendor,
    LogSdfModel,
    LogSdfFirmware,
    LogSdfFlags,
    LogSdfExtraField,
    LogSdfReadFailed,
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
    WarnPlatformMismatch,
    WarnCrossFlashConfirm,
    ReasonCrossFlashNotConfirmed,
    InfoTwoStepFlash,
    WarnFirmwareDowngrade,
    InfoFirmwareModelMismatch,
    ReasonMt1939NotCompatible,
    LogTruncated,
    StatusYes,
    StatusNo,
    DialogFilterFirmware,
    DialogFilterExecutable,
}

pub fn t(key: L10nKey, lang: Language) -> &'static str {
    match lang {
        Language::Auto | Language::English => en::t_en(key),
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
        assert_eq!(
            detect_system_language_from_locale(Some("xx-pt")),
            Language::Portuguese
        );
        assert_eq!(
            detect_system_language_from_locale(Some("zz_de")),
            Language::German
        );
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
        assert_eq!(resolve_language(Language::Auto), detect_system_language());
    }
    #[test]
    fn test_all_keys_non_empty_for_each_language() {
        for &lang in Language::ALL {
            if lang == Language::Auto {
                continue;
            }
            for key in L10nKey::ALL {
                assert!(!t(*key, lang).is_empty(), "{lang:?} missing {key:?}");
            }
        }
    }

    #[test]
    fn english_warnings_have_no_emoji_prefixes() {
        let keys = [
            L10nKey::LabelStopWarningMsg,
            L10nKey::LabelForceKillMsg,
            L10nKey::LabelExitWarningMsg,
            L10nKey::LabelFlashFailureMsg,
            L10nKey::WarnCannotCombine,
            L10nKey::WarnFirmwareLoadFailed,
        ];
        for key in keys {
            let s = t(key, Language::English);
            assert!(
                !s.contains('\u{26A0}'),
                "{key:?} still has warning emoji: {s}"
            );
        }
        assert_eq!(t(L10nKey::StatusYes, Language::English), "Yes");
        assert_eq!(t(L10nKey::StatusNo, Language::English), "No");
        assert_eq!(
            t(L10nKey::DialogFilterFirmware, Language::English),
            "Firmware"
        );
    }

    #[test]
    fn test_translation_with_args() {
        let args = [("required", "WRITE")];
        let translation = t_with_args(L10nKey::LabelTypeToConfirm, Language::English, &args);
        assert_eq!(translation, "Type \"WRITE\" to confirm:");
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
            ("ja", Language::English),
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
        let args = [("required", "FLASH H:")];
        let text = t_with_args(L10nKey::LabelTypeToConfirm, Language::English, &args);
        assert!(text.contains("FLASH H:"));
    }

    #[test]
    fn test_t_with_args_missing_placeholder() {
        let args = [("nonexistent", "value")];
        let text = t_with_args(L10nKey::TitleDriveProperties, Language::English, &args);
        assert_eq!(text, "Drive Properties");
    }

    #[test]
    fn test_language_all_count() {
        assert_eq!(Language::ALL.len(), 31);
    }

    #[test]
    fn test_translation_dispatch_returns_language_specific_text() {
        let en = t(L10nKey::TitleSettings, Language::English);
        let de = t(L10nKey::TitleSettings, Language::German);
        assert_eq!(en, "Settings");
        assert_eq!(de, "Einstellungen");
        assert_ne!(en, de, "German translation should differ from English");
    }

    #[test]
    fn test_auto_language_falls_back_to_english() {
        let en = t(L10nKey::TitleSettings, Language::English);
        let auto = t(L10nKey::TitleSettings, Language::Auto);
        assert_eq!(en, auto, "Language::Auto should fall back to English");
    }

    #[test]
    fn test_german_translations_complete() {
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
        L10nKey::LogParsedOneDriveFromOutput,
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
            let min_localized = LOG_KEYS.len() - 6;
            assert!(
                localized >= min_localized,
                "{lang:?} has too many English fallbacks for log keys ({localized}/{min_localized})"
            );
        }
    }
}
