/// Drive form factor classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveFormFactor {
    Desktop,
    Slim,
    Unknown,
}

impl DriveFormFactor {
    pub fn label(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Slim => "slim",
            Self::Unknown => "unknown",
        }
    }
}

/// Classify a drive from its product/model name.
pub fn classify_drive(model: &str) -> DriveFormFactor {
    let slim_models = [
        "BU40N", "BU50N", "BP71N", "WP50NB40", "BP50NB40", "BP55EB40", "BP60NB10", "BU20N", "BU30N",
    ];
    let desktop_models = [
        "BW-16D1HT",
        "BW-16D1X-U",
        "BC-12D2HT",
        "BC-12B1ST",
        "BW-12B1ST",
        "WH16NS60",
        "BH16NS60",
        "WH16NS40",
        "BH16NS40",
        "WH14NS40",
        "BH14NS40",
        "BH16NS55",
        "BH16NS50",
        "BH14NS50",
        "BH14NS58",
        "BH16NS58",
        "WH16NS58",
        "UH12NS40",
        "CH12NS40",
        "BH40N",
        "BH50N",
        "BE16NU50",
        "BH14NS48",
        "BH16NS48",
    ];
    for s in &slim_models {
        if model.contains(s) {
            return DriveFormFactor::Slim;
        }
    }
    for d in &desktop_models {
        if model.contains(d) {
            return DriveFormFactor::Desktop;
        }
    }
    DriveFormFactor::Unknown
}

/// Check if a drive model needs two-step flashing.
pub fn needs_two_step_flash(model: &str) -> bool {
    model.contains("BP50NB40") || model.contains("WP50NB40") || model.contains("BP55EB40")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_drive_desktop_models() {
        assert_eq!(classify_drive("BW-16D1HT"), DriveFormFactor::Desktop);
        assert_eq!(
            classify_drive("HL-DT-ST BW-16D1HT"),
            DriveFormFactor::Desktop
        );
        assert_eq!(classify_drive("WH16NS60"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("BH16NS60"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("WH16NS40"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("BH14NS40"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("BH16NS55"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("BC-12D2HT"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("BW-16D1X-U"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("BH14NS48"), DriveFormFactor::Desktop);
        assert_eq!(classify_drive("BH16NS48"), DriveFormFactor::Desktop);
    }

    #[test]
    fn classify_drive_slim_models() {
        assert_eq!(classify_drive("BU40N"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BD-RE BU40N"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BU50N"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BP71N"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("WP50NB40"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BP50NB40"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BP55EB40"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BP60NB10"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BU20N"), DriveFormFactor::Slim);
        assert_eq!(classify_drive("BU30N"), DriveFormFactor::Slim);
    }

    #[test]
    fn classify_drive_unknown_model() {
        assert_eq!(classify_drive("UNKNOWN-DRIVE"), DriveFormFactor::Unknown);
        assert_eq!(classify_drive(""), DriveFormFactor::Unknown);
        assert_eq!(classify_drive("SomeRandomDrive"), DriveFormFactor::Unknown);
    }

    #[test]
    fn classify_drive_slim_checked_before_desktop() {
        // Shorter slim names should not be confused with desktop names.
        assert_eq!(classify_drive("BU40N"), DriveFormFactor::Slim);
    }

    #[test]
    fn needs_two_step_flash_matching_models() {
        assert!(needs_two_step_flash("BP50NB40"));
        assert!(needs_two_step_flash("WP50NB40"));
        assert!(needs_two_step_flash("BP55EB40"));
        assert!(needs_two_step_flash("HL-DT-ST BP50NB40"));
        assert!(needs_two_step_flash("DE_LG_BP50NB40-NB50_1.03_MK.bin"));
    }

    #[test]
    fn needs_two_step_flash_non_matching() {
        assert!(!needs_two_step_flash("BU40N"));
        assert!(!needs_two_step_flash("WH16NS60"));
        assert!(!needs_two_step_flash(""));
        assert!(!needs_two_step_flash("BW-16D1HT"));
    }

    #[test]
    fn form_factor_label() {
        assert_eq!(DriveFormFactor::Desktop.label(), "desktop");
        assert_eq!(DriveFormFactor::Slim.label(), "slim");
        assert_eq!(DriveFormFactor::Unknown.label(), "unknown");
    }
}
