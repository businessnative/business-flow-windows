use crate::audio_in::list_mics;
use crate::autostart::set_launch_at_login;
use crate::input::{DictationJob, HookCmd};
use crate::paste::paste_text;
use crate::secrets::{self, KeyInfo};
use crate::store;
use crate::AppState;
use bnflow_core::{AIRouter, AppSettings, HistoryItem};
use crate::http_client::Net;
use serde::Serialize;
use std::sync::mpsc::Sender;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub settings: AppSettings,
    pub keys: Vec<KeyInfo>,
    pub mics: Vec<String>,
    pub history: Vec<HistoryItem>,
    pub platform: String,
    pub stt_note: String,
}

#[tauri::command]
pub fn get_state(state: tauri::State<AppState>) -> UiState {
    let settings = state.settings.lock().map(|s| s.clone()).unwrap_or_default();
    let history = state.history.lock().map(|h| h.clone()).unwrap_or_default();
    UiState {
        settings,
        keys: secrets::describe_keys(),
        mics: list_mics(),
        history,
        platform: platform_name().into(),
        stt_note: "A Claude nem tud hangot szöveggé alakítani. Az átíráshoz OpenAI- vagy Gemini-kulcs kell.".into(),
    }
}

#[tauri::command]
pub fn save_settings(state: tauri::State<AppState>, settings: AppSettings) -> Result<(), String> {
    if !settings.transcription_provider.can_transcribe() {
        return Err("Átíráshoz OpenAI-t vagy Geminit válassz. A Claude-nak nincs beszédfelismerése.".into());
    }
    store::save_settings(&settings)?;
    set_launch_at_login(settings.launch_at_login)?;
    let shortcuts = settings.shortcuts.clone();
    if let Ok(mut guard) = state.settings.lock() {
        *guard = settings;
    }
    let _ = state.hook.send(HookCmd::Reload(shortcuts));
    Ok(())
}

#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<(), String> {
    let provider = secrets::parse_provider(&provider).ok_or("Ismeretlen szolgáltató.")?;
    secrets::set_key(provider, &key)
}

#[tauri::command]
pub async fn test_api_key(provider: String, key: String) -> Result<(), String> {
    let provider = secrets::parse_provider(&provider).ok_or("Ismeretlen szolgáltató.")?;
    let stored = if key.trim().is_empty() { secrets::read_key(provider).unwrap_or_default() } else { key.trim().to_string() };
    if stored.is_empty() {
        return Err("Nincs kulcs.".into());
    }
    match AIRouter::new(Net).test_key(provider, &stored).await {
        Ok(()) => Ok(()),
        Err(err) => Err(err.message()),
    }
}

#[tauri::command]
pub fn clear_history(state: tauri::State<AppState>) -> Result<(), String> {
    store::clear_history()?;
    if let Ok(mut guard) = state.history.lock() {
        guard.clear();
    }
    Ok(())
}

#[tauri::command]
pub fn paste_history_item(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let text = state.history.lock().ok().and_then(|h| h.iter().find(|i| i.id.to_string() == id).map(|i| i.text.clone()));
    let Some(text) = text else { return Err("Nincs ilyen előzmény.".into()) };
    paste_text(&text)
}

#[tauri::command]
pub fn toggle_dictation(state: tauri::State<AppState>) -> Result<(), String> {
    state.hook.send(HookCmd::UiToggle).map_err(|_| "A billentyűfigyelő nem fut.".to_string())
}

#[allow(dead_code)]
pub fn jobs(state: &AppState) -> &Sender<DictationJob> {
    &state.jobs
}

fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}
