//! Product branding shared across GUI, CLI, and packaging.

/// User-facing application name shown in window titles, installers, and help text.
/// Sourced from `Cargo.toml` `[package.metadata.packager] product-name` via `build.rs`.
pub const DISPLAY_NAME: &str = env!("PRODUCT_NAME");

/// Official MakeMKV home page (About acknowledgements).
pub const MAKEMKV_HOME_URL: &str = "https://www.makemkv.com/";

/// Official MakeMKV download page (no-backend install guidance).
pub const MAKEMKV_DOWNLOAD_URL: &str = "https://www.makemkv.com/download/";

#[cfg(test)]
mod tests {
    use super::{DISPLAY_NAME, MAKEMKV_DOWNLOAD_URL, MAKEMKV_HOME_URL};

    #[test]
    fn display_name_matches_packager_product_name() {
        assert_eq!(DISPLAY_NAME, "SDF Flash GUI");
    }

    #[test]
    fn makemkv_urls_are_https_official() {
        assert!(MAKEMKV_HOME_URL.starts_with("https://www.makemkv.com"));
        assert!(MAKEMKV_DOWNLOAD_URL.starts_with("https://www.makemkv.com"));
        assert!(MAKEMKV_DOWNLOAD_URL.contains("download"));
    }
}
