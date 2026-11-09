use crate::audio_in::list_mics;
use crate::meetings_run;
use crate::autostart::set_launch_at_login;
use crate::input::{DictationJob, HookCmd};
use crate::paste::{copy_text, paste_text};
use crate::secrets::{self, KeyInfo};
use crate::store;
use crate::AppState;
use bnflow_core::{AIRouter, AppSettings, HistoryItem, Recovery};
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
    pub stats: bnflow_core::HistoryStats,
    pub onboarding_done_steps: usize,
    pub onboarding_total: usize,
    pub platform: String,
    pub stt_note: String,
    pub polish_presets: Vec<(String, String)>,
}

#[tauri::command]
pub fn get_state(state: tauri::State<AppState>) -> UiState {
    let settings = state.settings.lock().map(|s| s.clone()).unwrap_or_default();
    let history = state.history.lock().map(|h| h.clone()).unwrap_or_default();
    let stats = bnflow_core::HistoryLogic::stats(&history, now_ms());
    let has_key = secrets::load_keys().can_transcribe();
    let steps = bnflow_core::OnboardingState {
        has_key,
        microphone: !list_mics().is_empty(),
        accessibility: cfg!(target_os = "windows"),
        input_monitoring: true,
        has_dictated: !history.is_empty(),
    };
    UiState {
        settings,
        keys: secrets::describe_keys(),
        mics: list_mics(),
        history,
        stats,
        onboarding_done_steps: steps.done_count(),
        onboarding_total: 5,
        platform: platform_name().into(),
        stt_note: "A Claude nem tud hangot szöveggé alakítani. Az átíráshoz OpenAI- vagy Gemini-kulcs kell.".into(),
        polish_presets: bnflow_core::POLISH_PRESETS.iter().map(|p| (p.0.to_string(), p.1.to_string())).collect(),
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
pub fn set_named_secret(name: String, value: String) -> Result<(), String> {
    secrets::set_named(&name, &value)
}

#[tauri::command]
pub async fn preview_meetings(state: tauri::State<'_, AppState>) -> Result<Vec<meetings_run::Upcoming>, String> {
    let settings = state.settings.lock().map(|s| s.clone()).unwrap_or_default();
    meetings_run::upcoming(&settings).await
}

#[tauri::command]
pub fn list_meeting_notes() -> Vec<meetings_run::NoteCard> {
    meetings_run::list_notes()
}

#[tauri::command]
pub fn request_meeting_join(url: String, title: String) -> Result<String, String> {
    meetings_run::queue_join(&url, if title.trim().is_empty() { None } else { Some(title.trim()) })
}

#[tauri::command]
pub fn polish_now(state: tauri::State<AppState>) -> Result<(), String> {
    state.jobs.send(DictationJob::Polish).map_err(|_| "A polírozás nem indult el.".to_string())
}

#[tauri::command]
pub fn learn_correction(state: tauri::State<AppState>, later: String) -> Result<String, String> {
    let mut settings = state.settings.lock().map_err(|_| "A beállítás foglalt.".to_string())?;
    let history = state.history.lock().map_err(|_| "Az előzmény foglalt.".to_string())?;
    let Some(last) = history.iter().rev().find(|item| item.has_text()) else { return Err("Még nincs diktálás, amit javítani lehetne.".into()) };
    let Some((heard, term)) = bnflow_core::AutoLearn::correction(&last.text, &last.text, &later, &settings.dictionary) else {
        return Err("Egy szó eltérést várok a beillesztett mondathoz képest.".into());
    };
    bnflow_core::AutoLearn::learn(&heard, &term, &mut settings.dictionary);
    store::save_settings(&settings)?;
    Ok(format!("Szótárba: {heard} → {term}"))
}

#[tauri::command]
pub fn clear_history(state: tauri::State<AppState>) -> Result<(), String> {
    store::clear_history()?;
    if let Ok(mut guard) = state.history.lock() {
        guard.clear();
    }
    if let Ok(mut slot) = state.last_text.lock() {
        *slot = None;
    }
    Ok(())
}

#[tauri::command]
pub fn paste_history_item(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    paste_text(&history_text(&state, &id)?)
}

#[tauri::command]
pub fn copy_history_item(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    copy_text(&history_text(&state, &id)?)
}

#[tauri::command]
pub fn paste_last(state: tauri::State<AppState>) -> Result<(), String> {
    paste_text(&reusable(&state)?)
}

#[tauri::command]
pub fn copy_last(state: tauri::State<AppState>) -> Result<(), String> {
    copy_text(&reusable(&state)?)
}

#[tauri::command]
pub async fn retry_history_item(app: tauri::AppHandle, state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    crate::dictation::retry(app, state.history.clone(), state.settings.clone(), state.last_text.clone(), id).await
}

fn history_text(state: &AppState, id: &str) -> Result<String, String> {
    let text = state.history.lock().ok().and_then(|items| items.iter().find(|item| item.id.to_string() == id).map(|item| item.text.clone()));
    let Some(text) = text else { return Err("Nincs ilyen előzmény.".into()) };
    if text.trim().is_empty() {
        return Err("Ebben a sorban nincs szöveg.".into());
    }
    Ok(text)
}

fn reusable(state: &AppState) -> Result<String, String> {
    let explicit = state.last_text.lock().ok().and_then(|slot| slot.clone());
    let history = state.history.lock().map(|items| items.clone()).unwrap_or_default();
    Recovery::reusable_text(explicit.as_deref(), &history).ok_or_else(|| "Még nincs diktált szöveg, amit elő lehetne venni.".to_string())
}

#[tauri::command]
pub fn toggle_dictation(state: tauri::State<AppState>) -> Result<(), String> {
    state.hook.send(HookCmd::UiToggle).map_err(|_| "A billentyűfigyelő nem fut.".to_string())
}

#[allow(dead_code)]
pub fn jobs(state: &AppState) -> &Sender<DictationJob> {
    &state.jobs
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
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
