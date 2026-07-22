use crate::command::Backend;
use crate::i18n::{t, L10nKey, Language};

pub fn validate_tool_path(path: &str, backend: Backend, lang: Language) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err(t(L10nKey::ValPathEmpty, lang).to_string());
    }
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(t(L10nKey::ValFileNotExist, lang).to_string());
    }
    if !p.is_file() {
        return Err(t(L10nKey::ValPathNotFile, lang).to_string());
    }
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    match backend {
        Backend::SdfTool => {
            if !file_name.contains("sdftool") {
                return Err(t(L10nKey::ValMustContainSdftool, lang).to_string());
            }
        }
        Backend::MakeMkvCon => {
            if !file_name.contains("makemkvcon") && !file_name.contains("makemkv") {
                return Err(t(L10nKey::ValMustContainMakemkv, lang).to_string());
            }
        }
    }
    Ok(())
}

pub fn validate_sdf_path(path: &str, lang: Language) -> Result<(), String> {
    if path.trim().is_empty() {
        return Ok(());
    }
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(t(L10nKey::ValFileNotExist, lang).to_string());
    }
    if !p.is_file() {
        return Err(t(L10nKey::ValPathNotFile, lang).to_string());
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "bin" {
        return Err(t(L10nKey::ValExtMustBeBin, lang).to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Backend;
    use crate::i18n::Language;

    #[test]
    fn validate_tool_path_empty() {
        assert!(validate_tool_path("", Backend::SdfTool, Language::English).is_err());
        assert!(validate_tool_path("  ", Backend::SdfTool, Language::English).is_err());
        assert!(validate_tool_path("", Backend::MakeMkvCon, Language::English).is_err());
    }

    #[test]
    fn validate_tool_path_nonexistent() {
        assert!(
            validate_tool_path("/nonexistent/sdftool", Backend::SdfTool, Language::English)
                .is_err()
        );
    }

    #[test]
    fn validate_tool_path_sdftool_valid() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_tool");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdftool64");
        std::fs::write(&file, b"").unwrap();
        assert!(
            validate_tool_path(&file.to_string_lossy(), Backend::SdfTool, Language::English)
                .is_ok()
        );
        assert!(validate_tool_path(
            &file.to_string_lossy(),
            Backend::MakeMkvCon,
            Language::English
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_tool_path_makemkvcon_valid() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_mkv");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("makemkvcon64");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_tool_path(
            &file.to_string_lossy(),
            Backend::MakeMkvCon,
            Language::English
        )
        .is_ok());
        assert!(
            validate_tool_path(&file.to_string_lossy(), Backend::SdfTool, Language::English)
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_tool_path_wrong_name() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_wrong");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("some_other_tool");
        std::fs::write(&file, b"").unwrap();
        assert!(
            validate_tool_path(&file.to_string_lossy(), Backend::SdfTool, Language::English)
                .is_err()
        );
        assert!(validate_tool_path(
            &file.to_string_lossy(),
            Backend::MakeMkvCon,
            Language::English
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_sdf_path_empty() {
        assert!(validate_sdf_path("", Language::English).is_ok());
        assert!(validate_sdf_path("  ", Language::English).is_ok());
    }

    #[test]
    fn validate_sdf_path_nonexistent() {
        assert!(validate_sdf_path("/nonexistent/sdf.bin", Language::English).is_err());
    }

    #[test]
    fn validate_sdf_path_wrong_extension() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_ext");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdf.txt");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_sdf_path(&file.to_string_lossy(), Language::English).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_sdf_path_bin_extension() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_bin");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdf.bin");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_sdf_path(&file.to_string_lossy(), Language::English).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_sdf_path_case_insensitive_extension() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_case");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdf.BIN");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_sdf_path(&file.to_string_lossy(), Language::English).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_tool_path_is_directory() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_isdir");
        let _ = std::fs::create_dir_all(&dir);
        let err = validate_tool_path(&dir.to_string_lossy(), Backend::SdfTool, Language::English)
            .unwrap_err();
        assert!(err.contains("not a file"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_sdf_path_is_directory() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_sdfdir");
        let _ = std::fs::create_dir_all(&dir);
        let err = validate_sdf_path(&dir.to_string_lossy(), Language::English).unwrap_err();
        assert!(err.contains("not a file"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
