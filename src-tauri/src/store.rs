use crate::identity::APP_NAME;
use bnflow_core::{AppSettings, AudioFolder, HistoryItem, HistoryLogic, HistoryRetention};
use std::collections::HashSet;
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

pub fn audio_dir() -> PathBuf {
    app_dir().join("dictation-audio")
}

pub fn load_history() -> Vec<HistoryItem> {
    let path = app_dir().join("history.json");
    std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn load_history_kept(retention: HistoryRetention) -> Vec<HistoryItem> {
    let items = load_history();
    commit_history(&items, retention).unwrap_or(items)
}

/// Writes history and deletes audio that retention, the cap, or a missing reference no longer keeps.
pub fn commit_history(items: &[HistoryItem], retention: HistoryRetention) -> Result<Vec<HistoryItem>, String> {
    let (kept, drop_audio) = HistoryLogic::retain(items, retention, now_ms(), 400);
    let dir = app_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let body = serde_json::to_vec_pretty(&kept).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("history.json"), body).map_err(|e| e.to_string())?;
    let audio = audio_dir();
    for name in &drop_audio {
        let _ = AudioFolder::remove(&audio, name);
    }
    let keep: HashSet<String> = kept.iter().filter_map(|item| item.audio_name.clone()).collect();
    let _ = AudioFolder::sweep(&audio, &keep);
    Ok(kept)
}

pub fn write_audio(name: &str, bytes: &[u8]) -> Result<(), String> {
    AudioFolder::write(&audio_dir(), name, bytes)
}

pub fn clear_history() -> Result<(), String> {
    let path = app_dir().join("history.json");
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    let _ = AudioFolder::sweep(&audio_dir(), &HashSet::new());
    clear_last_transcript()
}

pub fn load_last_transcript() -> Option<String> {
    let text = std::fs::read_to_string(app_dir().join("last-transcript.txt")).ok()?;
    let text = text.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

pub fn save_last_transcript(text: &str) -> Result<(), String> {
    let dir = app_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("last-transcript.txt"), text).map_err(|e| e.to_string())
}

pub fn clear_last_transcript() -> Result<(), String> {
    let path = app_dir().join("last-transcript.txt");
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
