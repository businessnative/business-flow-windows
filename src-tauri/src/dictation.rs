use crate::audio_in::{self, AudioCmd};
use crate::foreground::capture_context;
use crate::http_client::{self, Net};
use crate::input::DictationJob;
use crate::paste::{capture_selection, paste_text};
use crate::secrets::load_keys;
use crate::store;
use async_trait::async_trait;
use bnflow_core::audio::{analyze, clean_transcript, encode_wav, float_to_i16, is_silent, prepare, resample_mono_16k};
use bnflow_core::{AIRouter, AppSettings, ChatProvider, DictationFormatter, FlowError, HistoryItem, HistoryKind, Prompts, ProviderKeys, TextPipeline, WordDiff, POLISH_PRESETS};
use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Phase {
    pub phase: String,
    pub message: String,
    pub level: f32,
}

pub struct Engine {
    settings: Arc<Mutex<AppSettings>>,
    history: Arc<Mutex<Vec<HistoryItem>>>,
    pub level: Arc<AtomicU32>,
    audio_tx: Sender<AudioCmd>,
    audio_rx: Receiver<audio_in::AudioReply>,
}

impl Engine {
    pub fn spawn(app: AppHandle, settings: Arc<Mutex<AppSettings>>, history: Arc<Mutex<Vec<HistoryItem>>>) -> Sender<DictationJob> {
        let level = Arc::new(AtomicU32::new(0));
        let (audio_tx, audio_rx) = audio_in::spawn(level.clone());
        let (jobs, job_rx) = mpsc::channel();
        let engine = Engine { settings, history, level, audio_tx, audio_rx };
        std::thread::Builder::new().name("dictation".into()).spawn(move || engine.run(app, job_rx)).ok();
        jobs
    }

    fn run(self, app: AppHandle, jobs: Receiver<DictationJob>) {
        let mut recording = false;
        let mut started = Instant::now();
        let mut hands_free = false;
        let mut command = false;
        loop {
            let job = if recording {
                match jobs.recv_timeout(Duration::from_millis(80)) {
                    Ok(job) => Some(job),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match jobs.recv() {
                    Ok(job) => Some(job),
                    Err(_) => break,
                }
            };
            if recording {
                let level = self.level.load(Ordering::Relaxed) as f32 / 1000.0;
                let label = if command { "Parancs" } else if hands_free { "Kihangosítva — beszélsz" } else { "Felvétel" };
                emit(&app, if hands_free { "handsfree" } else { "recording" }, label, level);
                if started.elapsed() > Duration::from_secs(180) {
                    let mode = command;
                    command = false;
                    self.finish(&app, true, mode);
                    recording = false;
                    continue;
                }
            }
            let Some(job) = job else { continue };
            match job {
                DictationJob::Start { hands_free: hf, command: is_command } => {
                    if recording {
                        continue;
                    }
                    if !load_keys().can_transcribe() {
                        emit(&app, "error", "Az átíráshoz OpenAI- vagy Gemini-kulcs kell. A Claude nem tud hangot szöveggé alakítani.", 0.0);
                        hide_later(&app, 4000);
                        continue;
                    }
                    http_client::warm_up();
                    let device = self.settings.lock().ok().and_then(|s| s.microphone_name.clone());
                    let _ = self.audio_tx.send(AudioCmd::Start { device });
                    recording = true;
                    hands_free = hf;
                    command = is_command;
                    started = Instant::now();
                    show_overlay(&app);
                    emit(&app, if hf { "handsfree" } else { "recording" }, if is_command { "Parancs — mondd az utasítást" } else if hf { "Kihangosítva — beszélsz" } else { "Felvétel" }, 0.0);
                }
                DictationJob::Stop => {
                    if recording {
                        if started.elapsed() < Duration::from_millis(180) && !hands_free {
                            let _ = self.audio_tx.send(AudioCmd::Cancel);
                            recording = false;
                            emit(&app, "idle", "", 0.0);
                            hide_overlay(&app);
                        } else {
                            let mode = command;
                            command = false;
                            self.finish(&app, true, mode);
                            recording = false;
                        }
                    }
                }
                DictationJob::Cancel => {
                    if recording {
                        let _ = self.audio_tx.send(AudioCmd::Cancel);
                        recording = false;
                    }
                    emit(&app, "idle", "", 0.0);
                    hide_overlay(&app);
                }
                DictationJob::PasteLast => {
                    if let Some(text) = self.history.lock().ok().and_then(|h| h.last().map(|i| i.text.clone())) {
                        let _ = paste_text(&text);
                    }
                }
                DictationJob::Polish => {
                    let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
                    let history = self.history.clone();
                    let app = app.clone();
                    show_overlay(&app);
                    emit(&app, "processing", "Polírozás…", 0.0);
                    std::thread::spawn(move || {
                        let selection = capture_selection();
                        tauri::async_runtime::spawn(async move {
                            let message = polish_selection(selection, settings, history).await;
                            emit(&app, &message.0, &message.1, 0.0);
                            hide_later(&app, 4000);
                        });
                    });
                }
            }
        }
    }

    fn finish(&self, app: &AppHandle, process: bool, command: bool) {
        let _ = self.audio_tx.send(AudioCmd::Stop);
        let reply = match self.audio_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(reply) => reply,
            Err(_) => {
                emit(app, "error", "A felvétel leállítása nem sikerült.", 0.0);
                hide_later(app, 2500);
                return;
            }
        };
        if !process {
            return;
        }
        emit(app, "processing", "Átírás…", 0.0);
        let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
        let history = self.history.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let message = process_audio(reply, settings, history, command).await;
            emit(&app, &message.0, &message.1, 0.0);
            hide_later(&app, if message.0 == "error" { 4500 } else { 1600 });
        });
    }
}

async fn process_audio(reply: audio_in::AudioReply, settings: AppSettings, history: Arc<Mutex<Vec<HistoryItem>>>, command: bool) -> (String, String) {
    let mono = resample_mono_16k(&reply.samples, reply.channels, reply.rate);
    if mono.is_empty() {
        return ("error".into(), "Nem érkezett hang. Ellenőrizd a mikrofont és az engedélyt.".into());
    }
    let prepared = prepare(&mono, 16_000);
    let stats = analyze(&prepared, 16_000);
    if is_silent(&stats) {
        return ("error".into(), "Nem hallottam beszédet.".into());
    }
    let wav = encode_wav(&float_to_i16(&prepared), 16_000);
    let keys = load_keys();
    if !keys.can_transcribe() {
        return ("error".into(), FlowError::NoApiKey.message());
    }
    let terms: Vec<String> = settings.dictionary.iter().map(|d| d.term.clone()).collect();
    let router = AIRouter::new(Net);
    let (result, _) = router.transcribe(&settings, &keys, &wav, &terms).await;
    let raw = match result {
        Ok(text) => text,
        Err(err) => return ("error".into(), err.message()),
    };
    let raw = clean_transcript(&raw, stats.peak, Some(stats.speech_seconds));
    if raw.trim().is_empty() {
        return ("error".into(), "Nem érkezett szöveg.".into());
    }
    let ctx = capture_context();
    let selection = if command { capture_selection() } else { None };
    let text = if command {
        let chat = RouterChat { settings: settings.clone(), keys: keys.clone() };
        match chat.complete(&Prompts::command(selection.is_some()), &Prompts::command_user(&raw, selection.as_deref()), None).await {
            Ok(out) => Prompts::clean_output(&out),
            Err(err) => return ("error".into(), err.message()),
        }
    } else {
        let chat: Option<RouterChat> = if settings.smart_formatting { Some(RouterChat { settings: settings.clone(), keys: keys.clone() }) } else { None };
        DictationFormatter::format(&raw, &settings, &ctx, chat.as_ref().map(|c| c as &dyn ChatProvider)).await.text
    };
    let mut text = text;
    if text.trim().is_empty() {
        return ("error".into(), "A szöveg üres lett a formázás után.".into());
    }
    if let Some(prev) = ctx.before.chars().last() {
        if TextPipeline::needs_leading_space(Some(prev), &text) {
            text.insert(0, ' ');
        }
    }
    if settings.history_enabled {
        let mut item = HistoryItem::with_kind(text.clone(), raw, ctx.app_name.clone(), stats.seconds, if command { HistoryKind::Command } else { HistoryKind::Dictation });
        if item.app_name.is_empty() {
            item.app_name = "Ismeretlen".into();
        }
        if let Ok(mut guard) = history.lock() {
            guard.push(item);
            let _ = store::save_history(&guard);
        }
    }
    match paste_text(&text) {
        Ok(()) => ("done".into(), text),
        Err(err) => ("error".into(), err),
    }
}

struct RouterChat {
    settings: AppSettings,
    keys: ProviderKeys,
}

#[async_trait]
impl ChatProvider for RouterChat {
    async fn complete(&self, system: &str, user: &str, prediction: Option<&str>) -> Result<String, FlowError> {
        AIRouter::new(Net).complete(&self.settings, &self.keys, system, user, prediction).await
    }
}

fn emit(app: &AppHandle, phase: &str, message: &str, level: f32) {
    let _ = app.emit("dictation", Phase { phase: phase.into(), message: message.into(), level });
}

fn show_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        place(&window);
        let _ = window.show();
    }
}

fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

async fn polish_selection(selection: Option<String>, settings: AppSettings, history: Arc<Mutex<Vec<HistoryItem>>>) -> (String, String) {
    let Some(selection) = selection else {
        return ("error".into(), "Nincs kijelölés. Jelöld ki a szöveget, majd nyomd meg a polírozás billentyűt.".into());
    };
    let keys = load_keys();
    if keys.is_empty() {
        return ("error".into(), "A polírozáshoz OpenAI, Claude vagy Gemini kulcs kell.".into());
    }
    let prompt = POLISH_PRESETS.iter().find(|p| p.0 == settings.default_polish_id).map(|p| p.2).unwrap_or(POLISH_PRESETS[0].2);
    let chat = RouterChat { settings: settings.clone(), keys };
    let out = match chat.complete(&Prompts::polish(prompt), &selection, None).await {
        Ok(out) => Prompts::clean_output(&out),
        Err(err) => return ("error".into(), err.message()),
    };
    let diff = WordDiff::diff(&selection, &out);
    let changes = diff.iter().filter(|op| !matches!(op, bnflow_core::DiffOp::Same(_))).count();
    if settings.history_enabled {
        if let Ok(mut guard) = history.lock() {
            guard.push(HistoryItem::with_kind(out.clone(), selection.clone(), "Polírozás".into(), 0.0, HistoryKind::Polish));
            let _ = store::save_history(&guard);
        }
    }
    match paste_text(&out) {
        Ok(()) => ("done".into(), format!("Kész · {changes} változás")),
        Err(err) => ("error".into(), err),
    }
}

fn hide_later(app: &AppHandle, ms: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        hide_overlay(&app);
        emit(&app, "idle", "", 0.0);
    });
}

fn place(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let x = (size.width as f64 / scale - 340.0) / 2.0;
        let y = size.height as f64 / scale - 120.0;
        let _ = window.set_position(tauri::LogicalPosition::new(x.max(12.0), y.max(12.0)));
    }
}
