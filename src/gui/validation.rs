// Path validation — shared between settings window and can_start/start_disabled_reason.

use crate::command::Backend;

pub fn validate_tool_path(path: &str, backend: Backend) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Path is empty".to_string());
    }
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err("File does not exist".to_string());
    }
    if !p.is_file() {
        return Err("Path is not a file".to_string());
    }
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    match backend {
        Backend::SdfTool => {
            if !file_name.contains("sdftool") {
                return Err("Filename must contain 'sdftool'".to_string());
            }
        }
        Backend::MakeMkvCon => {
            if !file_name.contains("makemkvcon") && !file_name.contains("makemkv") {
                return Err("Filename must contain 'makemkvcon' or 'makemkv'".to_string());
            }
        }
    }
    Ok(())
}

pub fn validate_sdf_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Ok(());
    }
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err("File does not exist".to_string());
    }
    if !p.is_file() {
        return Err("Path is not a file".to_string());
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "bin" {
        return Err("File extension must be '.bin'".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Backend;

    #[test]
    fn validate_tool_path_empty() {
        assert!(validate_tool_path("", Backend::SdfTool).is_err());
        assert!(validate_tool_path("  ", Backend::SdfTool).is_err());
        assert!(validate_tool_path("", Backend::MakeMkvCon).is_err());
    }

    #[test]
    fn validate_tool_path_nonexistent() {
        assert!(validate_tool_path("/nonexistent/sdftool", Backend::SdfTool).is_err());
    }

    #[test]
    fn validate_tool_path_sdftool_valid() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_tool");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdftool64");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_tool_path(&file.to_string_lossy(), Backend::SdfTool).is_ok());
        assert!(validate_tool_path(&file.to_string_lossy(), Backend::MakeMkvCon).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_tool_path_makemkvcon_valid() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_mkv");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("makemkvcon64");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_tool_path(&file.to_string_lossy(), Backend::MakeMkvCon).is_ok());
        assert!(validate_tool_path(&file.to_string_lossy(), Backend::SdfTool).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_tool_path_wrong_name() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_wrong");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("some_other_tool");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_tool_path(&file.to_string_lossy(), Backend::SdfTool).is_err());
        assert!(validate_tool_path(&file.to_string_lossy(), Backend::MakeMkvCon).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_sdf_path_empty() {
        assert!(validate_sdf_path("").is_ok());
        assert!(validate_sdf_path("  ").is_ok());
    }

    #[test]
    fn validate_sdf_path_nonexistent() {
        assert!(validate_sdf_path("/nonexistent/sdf.bin").is_err());
    }

    #[test]
    fn validate_sdf_path_wrong_extension() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_ext");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdf.txt");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_sdf_path(&file.to_string_lossy()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_sdf_path_bin_extension() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_bin");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdf.bin");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_sdf_path(&file.to_string_lossy()).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_sdf_path_case_insensitive_extension() {
        let dir = std::env::temp_dir().join("sdf_flash_test_validation_case");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sdf.BIN");
        std::fs::write(&file, b"").unwrap();
        assert!(validate_sdf_path(&file.to_string_lossy()).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
