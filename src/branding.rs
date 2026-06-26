//! Product branding shared across GUI, CLI, and packaging.
//!
//! Keep in sync with `Cargo.toml` `[package.metadata.packager] product-name` and
//! `build.rs` (Windows PE metadata).

/// User-facing application name shown in window titles, installers, and help text.
pub const DISPLAY_NAME: &str = "SDF Flash GUI";

#[cfg(test)]
mod tests {
    use super::DISPLAY_NAME;

    #[test]
    fn display_name_matches_packager_product_name() {
        assert_eq!(DISPLAY_NAME, "SDF Flash GUI");
    }
}
