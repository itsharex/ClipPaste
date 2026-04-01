use std::path::PathBuf;

pub fn get_config_path() -> PathBuf {
    let default_config_dir = match dirs::config_dir() {
        Some(path) => path.join("ClipPaste"),
        None => std::env::current_dir().unwrap_or(PathBuf::from(".")).join("ClipPaste"),
    };
    default_config_dir.join("config.json")
}

pub fn get_default_data_dir() -> PathBuf {
    let current_dir = std::env::current_dir().unwrap_or(PathBuf::from("."));
    match dirs::data_dir() {
        Some(path) => path.join("ClipPaste"),
        None => current_dir.join("ClipPaste"),
    }
}
