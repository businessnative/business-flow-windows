use crate::identity::APP_NAME;
use bnflow_core::{AppSettings, HistoryItem};
use std::path::PathBuf;

pub fn app_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_NAME)
}

pub fn load_settings() -> AppSettings {
    let path = app_dir().join("settings.json");
    let windows = cfg!(target_os = "windows");
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| AppSettings::fresh_for_platform(windows)),
        Err(_) => AppSettings::fresh_for_platform(windows),
    }
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let dir = app_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let body = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("settings.json"), body).map_err(|e| e.to_string())
}

pub fn load_history() -> Vec<HistoryItem> {
    let path = app_dir().join("history.json");
    std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_history(items: &[HistoryItem]) -> Result<(), String> {
    let dir = app_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let trimmed: Vec<_> = items.iter().rev().take(400).rev().cloned().collect();
    let body = serde_json::to_vec_pretty(&trimmed).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("history.json"), body).map_err(|e| e.to_string())
}

pub fn clear_history() -> Result<(), String> {
    let path = app_dir().join("history.json");
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
