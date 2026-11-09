//! Dictation recovery: keep the spoken attempt when transcription, the network, or paste fails.
//! Audio bytes stay on disk and are handed back only when History asks to retry.

use crate::formatter::FlowError;
use crate::models::{HistoryItem, HistoryKind, HistoryRetention, HistoryStatus};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use uuid::Uuid;

/// Taps shorter than this, with no detected speech and no transcript, are not stored.
pub const ACCIDENTAL_TAP_SECONDS: f64 = 0.5;

const AUDIO_NOTE: &str = "A hang a gépen maradt.";

#[derive(Debug, Clone, Copy)]
pub enum Outcome<'a> {
    Success,
    PasteFailed,
    Failed(&'a FlowError),
    Empty,
    Interrupted,
    NoSpeech,
    NoAudio,
}

pub struct Attempt<'a> {
    pub duration_seconds: f64,
    pub had_speech: bool,
    pub app_name: &'a str,
    pub kind: HistoryKind,
    pub raw_text: &'a str,
    pub text: &'a str,
    pub outcome: Outcome<'a>,
    pub history_enabled: bool,
    pub retention: HistoryRetention,
    pub has_wav: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub item: Option<HistoryItem>,
    pub remember_text: Option<String>,
    pub keep_audio: bool,
    pub discarded: bool,
    pub reason: String,
    pub status: HistoryStatus,
    pub log_line: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryUpdate {
    pub remember_text: Option<String>,
    pub delete_audio: Option<String>,
    pub reason: String,
    pub log_line: String,
}

struct Classified {
    status: HistoryStatus,
    reason: String,
    keep_audio: bool,
    produced_text: bool,
}

pub struct Recovery;

impl Recovery {
    pub fn should_keep(duration_seconds: f64, had_speech: bool, produced_text: bool) -> bool {
        if produced_text {
            return true;
        }
        duration_seconds >= ACCIDENTAL_TAP_SECONDS || had_speech
    }

    pub fn decide(attempt: &Attempt<'_>) -> Decision {
        let text = redact_secrets(attempt.text.trim());
        let raw = redact_secrets(attempt.raw_text.trim());
        let app = {
            let name = redact_secrets(attempt.app_name.trim());
            if name.is_empty() { "Ismeretlen".to_string() } else { name }
        };
        let mut class = classify(attempt.outcome, &text, attempt.has_wav);
        let discarded = !Self::should_keep(attempt.duration_seconds, attempt.had_speech, class.produced_text);
        let persist = attempt.history_enabled && attempt.retention != HistoryRetention::Never && !discarded;
        if !persist {
            class.keep_audio = false;
        }
        if class.keep_audio {
            class.reason = mention_audio(class.reason);
        }
        let remember = remember_text(class.status, &text);
        let log_line = if discarded {
            None
        } else {
            Some(log_line(if persist { class.status.token() } else { "unstored" }, class.keep_audio, &class.reason))
        };
        if !persist {
            return Decision {
                item: None,
                remember_text: remember,
                keep_audio: false,
                discarded,
                reason: class.reason,
                status: class.status,
                log_line,
            };
        }
        let mut item = HistoryItem::with_kind(text, raw, app, attempt.duration_seconds, attempt.kind);
        item.status = class.status;
        item.reason = class.reason.clone();
        if class.keep_audio {
            item.audio_name = Some(AudioFolder::name_for(item.id));
        }
        Decision {
            item: Some(item),
            remember_text: remember,
            keep_audio: class.keep_audio,
            discarded: false,
            reason: class.reason,
            status: class.status,
            log_line,
        }
    }

    /// Updates an existing row after Retry. The audio file name stays put when another retry is still possible.
    pub fn settle_retry(item: &mut HistoryItem, raw_text: &str, text: &str, outcome: Outcome<'_>) -> RetryUpdate {
        let previous_audio = item.audio_name.clone();
        let text_red = redact_secrets(text.trim());
        let raw_red = redact_secrets(raw_text.trim());
        let mut class = classify(outcome, &text_red, previous_audio.is_some());
        if class.keep_audio {
            class.reason = mention_audio(class.reason);
        }
        item.text = text_red.clone();
        item.raw_text = raw_red;
        item.status = class.status;
        item.reason = class.reason.clone();
        if class.keep_audio {
            item.audio_name = previous_audio.clone();
        } else {
            item.audio_name = None;
        }
        RetryUpdate {
            remember_text: remember_text(class.status, &text_red),
            delete_audio: if item.audio_name.is_none() { previous_audio } else { None },
            reason: class.reason.clone(),
            log_line: log_line(class.status.token(), item.audio_name.is_some(), &class.reason),
        }
    }

    pub fn reusable_text(explicit: Option<&str>, history: &[HistoryItem]) -> Option<String> {
        if let Some(text) = explicit.map(str::trim).filter(|text| !text.is_empty()) {
            return Some(text.to_string());
        }
        history.iter().rev().find(|item| item.has_text()).map(|item| item.text.clone())
    }

    pub fn retry_allowed(item: &HistoryItem) -> bool {
        matches!(item.status, HistoryStatus::Failed | HistoryStatus::Interrupted)
            && item.audio_name.as_deref().is_some_and(AudioFolder::is_safe_name)
    }

    pub fn redact_secrets(input: &str) -> String {
        redact_secrets(input)
    }

    pub fn load_for_retry(dir: &Path, item: &HistoryItem) -> Result<Vec<u8>, String> {
        if !Self::retry_allowed(item) {
            return Err("Ehhez a sorhoz nincs újrapróbálható hang.".into());
        }
        AudioFolder::read(dir, item.audio_name.as_deref().unwrap_or_default())
    }
}

fn classify(outcome: Outcome<'_>, text: &str, has_wav: bool) -> Classified {
    let produced_text = !text.trim().is_empty();
    let (status, reason, mut keep_audio) = match outcome {
        Outcome::Success if produced_text => (HistoryStatus::Success, String::new(), false),
        Outcome::Success => (HistoryStatus::Empty, "Nem érkezett szöveg.".into(), false),
        Outcome::PasteFailed => (HistoryStatus::PasteFailed, "A szöveg megvan, a beillesztés nem sikerült.".into(), false),
        Outcome::Empty => (HistoryStatus::Empty, "Nem érkezett szöveg.".into(), false),
        Outcome::NoSpeech => (HistoryStatus::Empty, "Nem hallottam beszédet.".into(), false),
        Outcome::NoAudio => (HistoryStatus::Interrupted, "Nem érkezett hang.".into(), false),
        Outcome::Interrupted => (HistoryStatus::Interrupted, "A diktálás megszakadt.".into(), true),
        Outcome::Failed(err) => {
            let status = if matches!(err, FlowError::Network | FlowError::Timeout) {
                HistoryStatus::Interrupted
            } else if matches!(err, FlowError::EmptyResult) && !produced_text {
                HistoryStatus::Empty
            } else {
                HistoryStatus::Failed
            };
            let reason = match err {
                FlowError::Network => "A kapcsolat megszakadt.".to_string(),
                FlowError::Timeout => "Az átírás időtúllépés miatt megszakadt.".to_string(),
                FlowError::EmptyResult if produced_text => "A szöveg üres lett a formázás után.".to_string(),
                FlowError::EmptyResult => "Nem érkezett szöveg.".to_string(),
                other => redact_secrets(&other.message()),
            };
            let keep = !matches!(err, FlowError::EmptyResult) && (!produced_text || err.is_retryable());
            (status, reason, keep)
        }
    };
    if !has_wav {
        keep_audio = false;
    }
    let produced_text = produced_text && !matches!(status, HistoryStatus::Empty);
    Classified { status, reason: redact_secrets(&reason), keep_audio, produced_text }
}

fn remember_text(status: HistoryStatus, text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    if matches!(status, HistoryStatus::Empty) {
        return None;
    }
    Some(text.to_string())
}

fn mention_audio(reason: String) -> String {
    if reason.is_empty() {
        return AUDIO_NOTE.to_string();
    }
    format!("{reason} {AUDIO_NOTE}")
}

fn log_line(status: &str, audio: bool, reason: &str) -> String {
    format!("history status={status} audio={} reason={}", if audio { 1 } else { 0 }, redact_secrets(reason))
}

fn redact_secrets(input: &str) -> String {
    let pattern = secret_pattern();
    let mut out = String::with_capacity(input.len());
    let mut last = 0;
    for found in pattern.find_iter(input) {
        let start = found.start();
        let boundary = start == 0 || !input.as_bytes()[start - 1].is_ascii_alphanumeric();
        if !boundary {
            continue;
        }
        out.push_str(&input[last..start]);
        out.push_str("[rejtett]");
        last = found.end();
    }
    out.push_str(&input[last..]);
    out
}

fn secret_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"(?i)bearer\s+[A-Za-z0-9\-._~+/]+=*|sk-ant-[A-Za-z0-9_\-]{8,}|sk-[A-Za-z0-9_\-]{8,}|AIza[0-9A-Za-z\-_]{16,}|(?:api[_-]?key|x-goog-api-key|x-api-key)\s*[:=]\s*\S+",
        )
        .expect("secret pattern")
    })
}

pub struct AudioFolder;

impl AudioFolder {
    pub fn name_for(id: Uuid) -> String {
        format!("{id}.wav")
    }

    pub fn is_safe_name(name: &str) -> bool {
        let Some(stem) = name.strip_suffix(".wav") else { return false };
        if stem.len() != 36 || stem.contains('.') || name.contains('/') || name.contains('\\') {
            return false;
        }
        Uuid::parse_str(stem).is_ok()
    }

    pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = file_in(dir, name)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| "A hangfájl nem menthető.".to_string())?;
        }
        std::fs::write(path, bytes).map_err(|_| "A hangfájl nem menthető.".to_string())
    }

    pub fn read(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
        let path = file_in(dir, name)?;
        std::fs::read(path).map_err(|_| "A hangfájl nem olvasható.".to_string())
    }

    pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
        let path = file_in(dir, name)?;
        if path.exists() {
            std::fs::remove_file(path).map_err(|_| "A hangfájl nem törölhető.".to_string())?;
        }
        Ok(())
    }

    /// Deletes safe wav names that are no longer referenced. Leaves every other file alone.
    pub fn sweep(dir: &Path, keep: &HashSet<String>) -> Result<(), String> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if Self::is_safe_name(&name) && !keep.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

fn file_in(dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !AudioFolder::is_safe_name(name) {
        return Err("Érvénytelen hangfájl.".into());
    }
    Ok(dir.join(name))
}
