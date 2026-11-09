use crate::audio_in::{self, AudioCmd};
use crate::foreground::capture_context;
use crate::http_client::{self, Net};
use crate::input::DictationJob;
use crate::paste::{capture_selection, paste_text};
use crate::secrets::load_keys;
use crate::store;
use async_trait::async_trait;
use bnflow_core::audio::{analyze, clean_transcript, encode_wav, float_to_i16, is_silent, prepare, resample_mono_16k};
use bnflow_core::{AIRouter, AppSettings, Attempt, ChatProvider, DictationFormatter, FlowError, HistoryItem, HistoryKind, HistoryStatus, Outcome, Prompts, ProviderKeys, Recovery, TextPipeline, WordDiff, POLISH_PRESETS};
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
    pub can_copy: bool,
}

struct Notice {
    phase: String,
    message: String,
    can_copy: bool,
}

pub struct Engine {
    settings: Arc<Mutex<AppSettings>>,
    history: Arc<Mutex<Vec<HistoryItem>>>,
    last_text: Arc<Mutex<Option<String>>>,
    pub level: Arc<AtomicU32>,
    audio_tx: Sender<AudioCmd>,
    audio_rx: Receiver<audio_in::AudioReply>,
}

impl Engine {
    pub fn spawn(
        app: AppHandle,
        settings: Arc<Mutex<AppSettings>>,
        history: Arc<Mutex<Vec<HistoryItem>>>,
        last_text: Arc<Mutex<Option<String>>>,
    ) -> Sender<DictationJob> {
        let level = Arc::new(AtomicU32::new(0));
        let (audio_tx, audio_rx) = audio_in::spawn(level.clone());
        let (jobs, job_rx) = mpsc::channel();
        let engine = Engine { settings, history, last_text, level, audio_tx, audio_rx };
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
                emit(&app, if hands_free { "handsfree" } else { "recording" }, label, level, false);
                if started.elapsed() > Duration::from_secs(180) {
                    let mode = command;
                    command = false;
                    self.finish(&app, true, mode, started.elapsed().as_secs_f64());
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
                        emit(&app, "error", "Az átíráshoz OpenAI- vagy Gemini-kulcs kell. A Claude nem tud hangot szöveggé alakítani.", 0.0, false);
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
                    emit(&app, if hf { "handsfree" } else { "recording" }, if is_command { "Parancs — mondd az utasítást" } else if hf { "Kihangosítva — beszélsz" } else { "Felvétel" }, 0.0, false);
                }
                DictationJob::Stop => {
                    if recording {
                        if started.elapsed() < Duration::from_millis(180) && !hands_free {
                            let _ = self.audio_tx.send(AudioCmd::Cancel);
                            recording = false;
                            emit(&app, "idle", "", 0.0, false);
                            hide_overlay(&app);
                        } else {
                            let mode = command;
                            command = false;
                            self.finish(&app, true, mode, started.elapsed().as_secs_f64());
                            recording = false;
                        }
                    }
                }
                DictationJob::Cancel => {
                    if recording {
                        let held = started.elapsed().as_secs_f64();
                        let mode = command;
                        recording = false;
                        command = false;
                        if held < bnflow_core::ACCIDENTAL_TAP_SECONDS {
                            let _ = self.audio_tx.send(AudioCmd::Cancel);
                            emit(&app, "idle", "", 0.0, false);
                            hide_overlay(&app);
                        } else {
                            self.finish_interrupted(&app, held, mode);
                        }
                    } else {
                        emit(&app, "idle", "", 0.0, false);
                        hide_overlay(&app);
                    }
                }
                DictationJob::PasteLast => {
                    if let Some(text) = self.reusable() {
                        match paste_text(&text) {
                            Ok(()) => {
                                emit(&app, "done", "Beillesztve.", 0.0, false);
                                hide_later(&app, 1200);
                            }
                            Err(err) => {
                                show_overlay(&app);
                                emit(&app, "error", &format!("{err} Az előzményekből is másolhatod."), 0.0, true);
                                hide_later(&app, 8000);
                            }
                        }
                    } else {
                        show_overlay(&app);
                        emit(&app, "error", "Még nincs diktált szöveg, amit be lehetne illeszteni.", 0.0, false);
                        hide_later(&app, 2500);
                    }
                }
                DictationJob::Polish => {
                    let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
                    let history = self.history.clone();
                    let last_text = self.last_text.clone();
                    let app = app.clone();
                    show_overlay(&app);
                    emit(&app, "processing", "Polírozás…", 0.0, false);
                    std::thread::spawn(move || {
                        let selection = capture_selection();
                        tauri::async_runtime::spawn(async move {
                            let message = polish_selection(selection, settings, history, last_text).await;
                            emit(&app, &message.phase, &message.message, 0.0, message.can_copy);
                            hide_later(&app, if message.can_copy { 8000 } else { 4000 });
                        });
                    });
                }
            }
        }
    }

    fn reusable(&self) -> Option<String> {
        let explicit = self.last_text.lock().ok().and_then(|slot| slot.clone());
        let history = self.history.lock().map(|items| items.clone()).unwrap_or_default();
        Recovery::reusable_text(explicit.as_deref(), &history)
    }

    fn finish(&self, app: &AppHandle, process: bool, command: bool, held: f64) {
        let _ = self.audio_tx.send(AudioCmd::Stop);
        let reply = match self.audio_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(reply) => reply,
            Err(_) => {
                let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
                let _ = save_attempt(&self.history, &self.last_text, &settings, interrupted_attempt(held, "Ismeretlen", command), &[]);
                emit(app, "error", "A felvétel leállítása nem sikerült.", 0.0, false);
                hide_later(app, 4500);
                return;
            }
        };
        if !process {
            return;
        }
        emit(app, "processing", "Átírás…", 0.0, false);
        let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
        let history = self.history.clone();
        let last_text = self.last_text.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let message = process_audio(reply, settings, history, last_text, command, held).await;
            emit(&app, &message.phase, &message.message, 0.0, message.can_copy);
            let wait = if message.phase == "error" { if message.can_copy { 8000 } else { 4500 } } else { 1600 };
            hide_later(&app, wait);
        });
    }

    fn finish_interrupted(&self, app: &AppHandle, held: f64, command: bool) {
        let _ = self.audio_tx.send(AudioCmd::Stop);
        let reply = self.audio_rx.recv_timeout(Duration::from_secs(2)).ok();
        let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
        let clip = reply.as_ref().map(clip_of).unwrap_or(Clip { wav: Vec::new(), had_speech: false, seconds: 0.0, peak: 0.0 });
        let ctx = capture_context();
        let mut attempt = interrupted_attempt(held.max(clip.seconds), &ctx.app_name, command);
        attempt.had_speech = clip.had_speech;
        let decision = save_attempt(&self.history, &self.last_text, &settings, attempt, &clip.wav);
        let notice = notice_saved(&decision);
        emit(app, &notice.phase, &notice.message, 0.0, notice.can_copy);
        hide_later(app, 4500);
    }
}

fn interrupted_attempt(duration: f64, app_name: &str, command: bool) -> Attempt<'_> {
    Attempt {
        duration_seconds: duration,
        had_speech: false,
        app_name,
        kind: if command { HistoryKind::Command } else { HistoryKind::Dictation },
        raw_text: "",
        text: "",
        outcome: Outcome::Interrupted,
        history_enabled: true,
        retention: bnflow_core::HistoryRetention::Forever,
        has_wav: false,
    }
}

struct Clip {
    wav: Vec<u8>,
    had_speech: bool,
    seconds: f64,
    peak: f32,
}

fn clip_of(reply: &audio_in::AudioReply) -> Clip {
    let mono = resample_mono_16k(&reply.samples, reply.channels, reply.rate);
    if mono.is_empty() {
        return Clip { wav: Vec::new(), had_speech: false, seconds: 0.0, peak: 0.0 };
    }
    let prepared = prepare(&mono, 16_000);
    let stats = analyze(&prepared, 16_000);
    let wav = encode_wav(&float_to_i16(&prepared), 16_000);
    Clip { wav, had_speech: !is_silent(&stats), seconds: stats.seconds, peak: stats.peak }
}

async fn process_audio(
    reply: audio_in::AudioReply,
    settings: AppSettings,
    history: Arc<Mutex<Vec<HistoryItem>>>,
    last_text: Arc<Mutex<Option<String>>>,
    command: bool,
    held: f64,
) -> Notice {
    let clip = clip_of(&reply);
    let duration = held.max(clip.seconds);
    let ctx = capture_context();
    let app_name = ctx.app_name.clone();
    if clip.wav.len() <= 44 {
        let decision = save_attempt(&history, &last_text, &settings, base_attempt(duration, clip.had_speech, &app_name, command, Outcome::NoAudio, "", ""), &clip.wav);
        return notice_saved(&decision);
    }
    if !clip.had_speech {
        let decision = save_attempt(&history, &last_text, &settings, base_attempt(duration, false, &app_name, command, Outcome::NoSpeech, "", ""), &clip.wav);
        return notice_saved(&decision);
    }
    let keys = load_keys();
    if !keys.can_transcribe() {
        let err = FlowError::NoApiKey;
        let decision = save_attempt(&history, &last_text, &settings, base_attempt(duration, true, &app_name, command, Outcome::Failed(&err), "", ""), &clip.wav);
        return notice_saved(&decision);
    }
    let terms: Vec<String> = settings.dictionary.iter().map(|d| d.term.clone()).collect();
    let (result, _) = AIRouter::new(Net).transcribe(&settings, &keys, &clip.wav, &terms).await;
    let raw = match result {
        Ok(text) => text,
        Err(err) => {
            let decision = save_attempt(&history, &last_text, &settings, base_attempt(duration, true, &app_name, command, Outcome::Failed(&err), "", ""), &clip.wav);
            return notice_saved(&decision);
        }
    };
    let raw = clean_transcript(&raw, clip.peak, Some(clip.seconds));
    if raw.trim().is_empty() {
        let decision = save_attempt(&history, &last_text, &settings, base_attempt(duration, true, &app_name, command, Outcome::Empty, "", ""), &clip.wav);
        return notice_saved(&decision);
    }
    let text = if command {
        let selection = capture_selection();
        let chat = RouterChat { settings: settings.clone(), keys: keys.clone() };
        match chat.complete(&Prompts::command(selection.is_some()), &Prompts::command_user(&raw, selection.as_deref()), None).await {
            Ok(out) => Prompts::clean_output(&out),
            Err(err) => {
                let decision = save_attempt(&history, &last_text, &settings, base_attempt(duration, true, &app_name, true, Outcome::Failed(&err), &raw, &raw), &clip.wav);
                return notice_saved(&decision);
            }
        }
    } else {
        let chat: Option<RouterChat> = if settings.smart_formatting { Some(RouterChat { settings: settings.clone(), keys: keys.clone() }) } else { None };
        DictationFormatter::format(&raw, &settings, &ctx, chat.as_ref().map(|c| c as &dyn ChatProvider)).await.text
    };
    if text.trim().is_empty() {
        let err = FlowError::EmptyResult;
        let decision = save_attempt(&history, &last_text, &settings, base_attempt(duration, true, &app_name, command, Outcome::Failed(&err), &raw, &raw), &clip.wav);
        return notice_saved(&decision);
    }
    let mut text = text;
    if let Some(prev) = ctx.before.chars().last() {
        if TextPipeline::needs_leading_space(Some(prev), &text) {
            text.insert(0, ' ');
        }
    }
    match paste_text(&text) {
        Ok(()) => {
            let _ = save_attempt(&history, &last_text, &settings, base_attempt(duration, true, &app_name, command, Outcome::Success, &raw, &text), &[]);
            Notice { phase: "done".into(), message: text, can_copy: false }
        }
        Err(err) => {
            let _ = save_attempt(&history, &last_text, &settings, base_attempt(duration, true, &app_name, command, Outcome::PasteFailed, &raw, &text), &[]);
            Notice { phase: "error".into(), message: format!("{err} Az előzményekből is másolhatod."), can_copy: true }
        }
    }
}

fn base_attempt<'a>(
    duration: f64,
    had_speech: bool,
    app_name: &'a str,
    command: bool,
    outcome: Outcome<'a>,
    raw_text: &'a str,
    text: &'a str,
) -> Attempt<'a> {
    Attempt {
        duration_seconds: duration,
        had_speech,
        app_name,
        kind: if command { HistoryKind::Command } else { HistoryKind::Dictation },
        raw_text,
        text,
        outcome,
        history_enabled: true,
        retention: bnflow_core::HistoryRetention::Forever,
        has_wav: false,
    }
}

fn save_attempt(
    history: &Mutex<Vec<HistoryItem>>,
    last_text: &Mutex<Option<String>>,
    settings: &AppSettings,
    mut attempt: Attempt<'_>,
    wav: &[u8],
) -> bnflow_core::Decision {
    attempt.has_wav = wav.len() > 44;
    attempt.history_enabled = settings.history_enabled;
    attempt.retention = settings.retention;
    let decision = Recovery::decide(&attempt);
    if decision.keep_audio {
        if let Some(name) = decision.item.as_ref().and_then(|item| item.audio_name.clone()) {
            let _ = store::write_audio(&name, wav);
        }
    }
    if let Some(item) = &decision.item {
        if let Ok(mut guard) = history.lock() {
            guard.push(item.clone());
            if let Ok(kept) = store::commit_history(&guard, settings.retention) {
                *guard = kept;
            }
        }
    }
    if let Some(text) = &decision.remember_text {
        if let Ok(mut slot) = last_text.lock() {
            *slot = Some(text.clone());
        }
        let _ = store::save_last_transcript(text);
    }
    if let Some(line) = &decision.log_line {
        eprintln!("{line}");
    }
    decision
}

fn notice_saved(decision: &bnflow_core::Decision) -> Notice {
    if decision.status == HistoryStatus::Success {
        return Notice { phase: "done".into(), message: decision.remember_text.clone().unwrap_or_default(), can_copy: false };
    }
    Notice {
        phase: "error".into(),
        message: decision.reason.clone(),
        can_copy: decision.remember_text.is_some(),
    }
}

pub async fn retry(
    app: AppHandle,
    history: Arc<Mutex<Vec<HistoryItem>>>,
    settings_slot: Arc<Mutex<AppSettings>>,
    last_text: Arc<Mutex<Option<String>>>,
    id: String,
) -> Result<(), String> {
    let settings = settings_slot.lock().map(|s| s.clone()).unwrap_or_default();
    let item = {
        let guard = history.lock().map_err(|_| "Az előzmény foglalt.".to_string())?;
        guard.iter().find(|item| item.id.to_string() == id).cloned().ok_or_else(|| "Nincs ilyen előzmény.".to_string())?
    };
    let wav = Recovery::load_for_retry(&store::audio_dir(), &item)?;
    emit(&app, "processing", "Újrapróbálás…", 0.0, false);
    show_overlay(&app);
    let keys = load_keys();
    if !keys.can_transcribe() {
        return settle_retry_error(&app, &history, &last_text, &settings, item, &FlowError::NoApiKey, "", "");
    }
    let terms: Vec<String> = settings.dictionary.iter().map(|d| d.term.clone()).collect();
    let (result, _) = AIRouter::new(Net).transcribe(&settings, &keys, &wav, &terms).await;
    let raw = match result {
        Ok(text) => clean_transcript(&text, 0.5, Some(item.duration_seconds)),
        Err(err) => return settle_retry_error(&app, &history, &last_text, &settings, item, &err, "", ""),
    };
    if raw.trim().is_empty() {
        return settle_retry_outcome(&app, &history, &last_text, &settings, item, "", "", Outcome::Empty, None);
    }
    let text = if item.kind == HistoryKind::Command {
        let chat = RouterChat { settings: settings.clone(), keys: keys.clone() };
        match chat.complete(&Prompts::command(false), &Prompts::command_user(&raw, None), None).await {
            Ok(out) => Prompts::clean_output(&out),
            Err(err) => return settle_retry_error(&app, &history, &last_text, &settings, item, &err, &raw, &raw),
        }
    } else {
        let ctx = capture_context();
        let chat: Option<RouterChat> = if settings.smart_formatting { Some(RouterChat { settings: settings.clone(), keys: keys.clone() }) } else { None };
        DictationFormatter::format(&raw, &settings, &ctx, chat.as_ref().map(|c| c as &dyn ChatProvider)).await.text
    };
    if text.trim().is_empty() {
        let err = FlowError::EmptyResult;
        return settle_retry_error(&app, &history, &last_text, &settings, item, &err, &raw, &raw);
    }
    let mut text = text;
    let ctx = capture_context();
    if let Some(prev) = ctx.before.chars().last() {
        if TextPipeline::needs_leading_space(Some(prev), &text) {
            text.insert(0, ' ');
        }
    }
    let pasted = paste_text(&text);
    let outcome = if pasted.is_ok() { Outcome::Success } else { Outcome::PasteFailed };
    let notice_err = pasted.err();
    settle_retry_outcome(&app, &history, &last_text, &settings, item, &raw, &text, outcome, notice_err)
}

fn settle_retry_error(
    app: &AppHandle,
    history: &Mutex<Vec<HistoryItem>>,
    last_text: &Mutex<Option<String>>,
    settings: &AppSettings,
    item: HistoryItem,
    err: &FlowError,
    raw: &str,
    text: &str,
) -> Result<(), String> {
    settle_retry_outcome(app, history, last_text, settings, item, raw, text, Outcome::Failed(err), None)
}

fn settle_retry_outcome(
    app: &AppHandle,
    history: &Mutex<Vec<HistoryItem>>,
    last_text: &Mutex<Option<String>>,
    settings: &AppSettings,
    mut item: HistoryItem,
    raw: &str,
    text: &str,
    outcome: Outcome<'_>,
    paste_error: Option<String>,
) -> Result<(), String> {
    let update = Recovery::settle_retry(&mut item, raw, text, outcome);
    if let Some(remembered) = &update.remember_text {
        if let Ok(mut slot) = last_text.lock() {
            *slot = Some(remembered.clone());
        }
        let _ = store::save_last_transcript(remembered);
    }
    eprintln!("{}", update.log_line);
    let id = item.id;
    if let Ok(mut guard) = history.lock() {
        if let Some(slot) = guard.iter_mut().find(|row| row.id == id) {
            *slot = item.clone();
        }
        if let Ok(kept) = store::commit_history(&guard, settings.retention) {
            *guard = kept;
        }
    }
    let failed = !matches!(item.status, HistoryStatus::Success);
    if failed {
        let message = match &paste_error {
            Some(err) => format!("{err} Az előzményekből is másolhatod."),
            None => update.reason.clone(),
        };
        emit(app, "error", &message, 0.0, update.remember_text.is_some());
        hide_later(app, if update.remember_text.is_some() { 8000 } else { 4500 });
        return Err(message);
    }
    emit(app, "done", &item.text, 0.0, false);
    hide_later(app, 1600);
    Ok(())
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

fn emit(app: &AppHandle, phase: &str, message: &str, level: f32, can_copy: bool) {
    let _ = app.emit("dictation", Phase { phase: phase.into(), message: message.into(), level, can_copy });
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

async fn polish_selection(
    selection: Option<String>,
    settings: AppSettings,
    history: Arc<Mutex<Vec<HistoryItem>>>,
    last_text: Arc<Mutex<Option<String>>>,
) -> Notice {
    let Some(selection) = selection else {
        return Notice { phase: "error".into(), message: "Nincs kijelölés. Jelöld ki a szöveget, majd nyomd meg a polírozás billentyűt.".into(), can_copy: false };
    };
    let keys = load_keys();
    if keys.is_empty() {
        return Notice { phase: "error".into(), message: "A polírozáshoz OpenAI, Claude vagy Gemini kulcs kell.".into(), can_copy: false };
    }
    let prompt = POLISH_PRESETS.iter().find(|p| p.0 == settings.default_polish_id).map(|p| p.2).unwrap_or(POLISH_PRESETS[0].2);
    let chat = RouterChat { settings: settings.clone(), keys };
    let out = match chat.complete(&Prompts::polish(prompt), &selection, None).await {
        Ok(out) => Prompts::clean_output(&out),
        Err(err) => return Notice { phase: "error".into(), message: err.message(), can_copy: false },
    };
    let diff = WordDiff::diff(&selection, &out);
    let changes = diff.iter().filter(|op| !matches!(op, bnflow_core::DiffOp::Same(_))).count();
    let attempt = Attempt {
        duration_seconds: 0.0,
        had_speech: true,
        app_name: "Polírozás",
        kind: HistoryKind::Polish,
        raw_text: &selection,
        text: &out,
        outcome: Outcome::Success,
        history_enabled: true,
        retention: bnflow_core::HistoryRetention::Forever,
        has_wav: false,
    };
    match paste_text(&out) {
        Ok(()) => {
            let _ = save_attempt(&history, &last_text, &settings, attempt, &[]);
            Notice { phase: "done".into(), message: format!("Kész · {changes} változás"), can_copy: false }
        }
        Err(err) => {
            let mut failed = attempt;
            failed.outcome = Outcome::PasteFailed;
            let _ = save_attempt(&history, &last_text, &settings, failed, &[]);
            Notice { phase: "error".into(), message: format!("{err} Az előzményekből is másolhatod."), can_copy: true }
        }
    }
}

fn hide_later(app: &AppHandle, ms: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        hide_overlay(&app);
        emit(&app, "idle", "", 0.0, false);
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
