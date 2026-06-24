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
