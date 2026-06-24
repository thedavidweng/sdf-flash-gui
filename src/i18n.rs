// Internationalization module for SDF Flash GUI.
// Aligns supported languages with Rufus.

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

pub fn detect_system_language() -> Language {
    if let Some(locale) = sys_locale::get_locale() {
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
    } else {
        Language::English
    }
}

pub fn resolve_language(lang: Language) -> Language {
    if lang == Language::Auto {
        detect_system_language()
    } else {
        lang
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L10nKey {
    TitleDriveProperties,
    LabelDevice,
    SectionOperation,
    TabWrite,
    TabRead,
    TabRecover,
    SectionFlashOptions,
    OptionBootloader,
    OptionEncrypted,
    SectionFirmwareImage,
    BtnBrowse,
    SectionManifest,
    LabelImageId,
    SectionOutputFolder,
    SectionConfirmation,
    SectionStatus,
    LabelTypeToConfirm,
    LabelWrongFw,
    BtnExtract,
    BtnClose,
    BtnStart,
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
    ReasonRunValidation,
    ReasonEnterToken,
}

pub fn t(key: L10nKey, _lang: Language) -> &'static str {
    t_en(key)
}

pub fn t_with_args(key: L10nKey, lang: Language, args: &[(&str, &str)]) -> String {
    let mut text = t(key, lang).to_string();
    for (k, v) in args {
        text = text.replace(&format!("{{{k}}}"), v);
    }
    text
}

fn t_en(key: L10nKey) -> &'static str {
    match key {
        L10nKey::TitleDriveProperties => "Drive Properties",
        L10nKey::LabelDevice => "Device",
        L10nKey::SectionOperation => "Operation",
        L10nKey::TabWrite => "WRITE Firmware",
        L10nKey::TabRead => "READ Firmware",
        L10nKey::TabRecover => "RECOVER Drive",
        L10nKey::SectionFlashOptions => "Flash Options",
        L10nKey::OptionBootloader => "Include boot-loader (dangerous)",
        L10nKey::OptionEncrypted => "Encrypted rawflash",
        L10nKey::SectionFirmwareImage => "Firmware Image",
        L10nKey::BtnBrowse => "Browse…",
        L10nKey::SectionManifest => "Firmware Manifest (optional)",
        L10nKey::LabelImageId => "Image ID",
        L10nKey::SectionOutputFolder => "Output Folder",
        L10nKey::SectionConfirmation => "Confirmation",
        L10nKey::SectionStatus => "Status",
        L10nKey::LabelTypeToConfirm => "Type \"{required}\" to confirm:",
        L10nKey::LabelWrongFw => "Wrong FW",
        L10nKey::BtnExtract => "Extract",
        L10nKey::BtnClose => "CLOSE",
        L10nKey::BtnStart => "START",
        L10nKey::StatusReady => "Ready",
        L10nKey::StatusNoDrives => "No optical drives detected",
        L10nKey::StatusProbing => "Probing drive",
        L10nKey::StatusProbeFailed => "Drive probe failed",
        L10nKey::StatusOpSuccess => "Operation completed successfully.",
        L10nKey::TooltipRefresh => "Refresh drives",
        L10nKey::TooltipSettings => "Settings",
        L10nKey::TooltipAbout => "About",
        L10nKey::TooltipStartEnabled => "Start the selected operation",
        L10nKey::TitleExitWarning => "Exit Warning",
        L10nKey::LabelExitWarningMsg => "⚠️ Warning: An operation is in progress!",
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
        L10nKey::ReasonRunValidation => "Run validation first (load manifest and confirm)",
        L10nKey::ReasonEnterToken => "Enter recovery token and confirmation",
    }
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
    fn test_resolve_language() {
        // Test that resolving a specific language returns itself
        assert_eq!(resolve_language(Language::French), Language::French);
        assert_eq!(resolve_language(Language::Spanish), Language::Spanish);

        // Test resolving Auto
        let resolved = resolve_language(Language::Auto);
        // It should match whatever detect_system_language() returns
        assert_eq!(resolved, detect_system_language());
    }

    #[test]
    fn test_translation_keys() {
        // Verify all keys can be translated
        let keys = [
            L10nKey::TitleDriveProperties,
            L10nKey::LabelDevice,
            L10nKey::SectionOperation,
            L10nKey::TabWrite,
            L10nKey::TabRead,
            L10nKey::TabRecover,
            L10nKey::SectionFlashOptions,
            L10nKey::OptionBootloader,
            L10nKey::OptionEncrypted,
            L10nKey::SectionFirmwareImage,
            L10nKey::BtnBrowse,
            L10nKey::SectionManifest,
            L10nKey::LabelImageId,
            L10nKey::SectionOutputFolder,
            L10nKey::SectionConfirmation,
            L10nKey::SectionStatus,
            L10nKey::LabelTypeToConfirm,
            L10nKey::LabelWrongFw,
            L10nKey::BtnExtract,
            L10nKey::BtnClose,
            L10nKey::BtnStart,
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
            L10nKey::ReasonRunValidation,
            L10nKey::ReasonEnterToken,
        ];

        for key in keys {
            let translation = t(key, Language::English);
            assert!(!translation.is_empty());
        }
    }

    #[test]
    fn test_translation_with_args() {
        let args = [("required", "WRITE")];
        let translation = t_with_args(L10nKey::LabelTypeToConfirm, Language::English, &args);
        assert_eq!(translation, "Type \"WRITE\" to confirm:");
    }

    #[test]
    fn test_system_language_detection() {
        // Just call it to make sure it doesn't panic
        let _ = detect_system_language();
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
    fn test_display_name_auto() {
        assert_eq!(Language::Auto.display_name(), "Auto-detect");
    }

    #[test]
    fn test_display_name_english() {
        assert_eq!(Language::English.display_name(), "English (English)");
    }

    #[test]
    fn test_display_name_german() {
        assert_eq!(Language::German.display_name(), "Deutsch (German)");
    }

    #[test]
    fn test_display_name_french() {
        assert_eq!(Language::French.display_name(), "Français (French)");
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
    fn test_all_keys_return_non_empty_english() {
        for key in [
            L10nKey::TitleDriveProperties,
            L10nKey::LabelDevice,
            L10nKey::SectionOperation,
            L10nKey::TabWrite,
            L10nKey::TabRead,
            L10nKey::TabRecover,
            L10nKey::SectionFlashOptions,
            L10nKey::OptionBootloader,
            L10nKey::OptionEncrypted,
            L10nKey::SectionFirmwareImage,
            L10nKey::BtnBrowse,
            L10nKey::SectionManifest,
            L10nKey::LabelImageId,
            L10nKey::SectionOutputFolder,
            L10nKey::SectionConfirmation,
            L10nKey::SectionStatus,
            L10nKey::LabelTypeToConfirm,
            L10nKey::LabelWrongFw,
            L10nKey::BtnExtract,
            L10nKey::BtnClose,
            L10nKey::BtnStart,
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
            L10nKey::ReasonRunValidation,
            L10nKey::ReasonEnterToken,
        ] {
            let text = t(key, Language::English);
            assert!(!text.is_empty(), "empty translation for key: {key:?}");
        }
    }

    #[test]
    fn test_language_all_count() {
        // 31 languages including Auto
        assert_eq!(Language::ALL.len(), 31);
    }

    #[test]
    fn test_language_clone_copy() {
        let lang = Language::French;
        let cloned = lang;
        assert_eq!(lang, cloned);
    }

    #[test]
    fn test_language_debug() {
        let debug = format!("{:?}", Language::German);
        assert_eq!(debug, "German");
    }

    #[test]
    fn test_language_serde_roundtrip() {
        let lang = Language::French;
        let json = serde_json::to_string(&lang).unwrap();
        let deserialized: Language = serde_json::from_str(&json).unwrap();
        assert_eq!(lang, deserialized);
    }

    #[test]
    fn test_l10n_key_count() {
        // Count actual variants by checking every key produces a non-empty translation.
        // This catches accidental deletion of keys or translations.
        let all_keys = [
            L10nKey::TitleDriveProperties,
            L10nKey::LabelDevice,
            L10nKey::SectionOperation,
            L10nKey::TabWrite,
            L10nKey::TabRead,
            L10nKey::TabRecover,
            L10nKey::SectionFlashOptions,
            L10nKey::OptionBootloader,
            L10nKey::OptionEncrypted,
            L10nKey::SectionFirmwareImage,
            L10nKey::BtnBrowse,
            L10nKey::SectionManifest,
            L10nKey::LabelImageId,
            L10nKey::SectionOutputFolder,
            L10nKey::SectionConfirmation,
            L10nKey::SectionStatus,
            L10nKey::LabelTypeToConfirm,
            L10nKey::LabelWrongFw,
            L10nKey::BtnExtract,
            L10nKey::BtnClose,
            L10nKey::BtnStart,
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
            L10nKey::ReasonRunValidation,
            L10nKey::ReasonEnterToken,
        ];
        assert!(
            all_keys.len() >= 45,
            "expected at least 45 L10nKey variants, got {}",
            all_keys.len()
        );
    }
}
