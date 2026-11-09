use bnflow_core::*;
use std::collections::HashSet;

const KEY: &str = "sk-proj-abcDEF1234567890xyz";
const GEMINI: &str = "AIzaSyA1234567890abcdefghij";

fn attempt<'a>(outcome: Outcome<'a>, text: &'a str, raw: &'a str) -> Attempt<'a> {
    Attempt {
        duration_seconds: 2.4,
        had_speech: true,
        app_name: "Jegyzettömb",
        kind: HistoryKind::Dictation,
        raw_text: raw,
        text,
        outcome,
        history_enabled: true,
        retention: HistoryRetention::Forever,
        has_wav: true,
    }
}

fn temp_audio() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bf-recovery-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn successful_dictation_is_stored_without_audio() {
    let decision = Recovery::decide(&attempt(Outcome::Success, "Holnap reggel indulunk.", "holnap reggel indulunk"));
    let item = decision.item.expect("row");
    assert_eq!(item.status, HistoryStatus::Success);
    assert!(item.reason.is_empty());
    assert!(item.audio_name.is_none());
    assert!(!decision.keep_audio);
    assert_eq!(decision.remember_text.as_deref(), Some("Holnap reggel indulunk."));
    let log = decision.log_line.expect("log");
    assert!(log.contains("status=success"));
    assert!(log.contains("audio=0"));
    assert!(!log.contains("Holnap"));
    let json = serde_json::to_string(&item).unwrap();
    assert!(json.contains("Holnap reggel indulunk."));
    assert!(!json.contains(KEY));
}

#[test]
fn failure_is_saved_with_hungarian_reason_and_audio() {
    let err = FlowError::Network;
    let decision = Recovery::decide(&attempt(Outcome::Failed(&err), "", ""));
    let item = decision.item.expect("failed row");
    assert_eq!(item.status, HistoryStatus::Interrupted);
    assert!(item.reason.contains("A kapcsolat megszakadt."));
    assert!(item.reason.contains("A hang a gépen maradt."));
    assert!(decision.keep_audio);
    assert!(item.audio_name.as_deref().is_some_and(AudioFolder::is_safe_name));
    assert!(decision.remember_text.is_none());
    assert!(Recovery::retry_allowed(&item));
    let json = serde_json::to_string(&item).unwrap();
    assert!(!json.contains("sk-"));
    assert!(!decision.log_line.unwrap().contains("sk-"));
}

#[test]
fn quota_failure_keeps_audio_and_the_existing_sentence() {
    let err = FlowError::Quota;
    let decision = Recovery::decide(&attempt(Outcome::Failed(&err), "", ""));
    let item = decision.item.expect("quota row");
    assert_eq!(item.status, HistoryStatus::Failed);
    assert!(item.reason.contains("Elfogyott az átírás kerete."));
    assert!(decision.keep_audio);
    assert!(Recovery::retry_allowed(&item));
}

#[test]
fn empty_transcript_is_saved_without_retry_audio() {
    let decision = Recovery::decide(&attempt(Outcome::Empty, "", ""));
    let item = decision.item.expect("empty row");
    assert_eq!(item.status, HistoryStatus::Empty);
    assert_eq!(item.reason, "Nem érkezett szöveg.");
    assert!(item.audio_name.is_none());
    assert!(!Recovery::retry_allowed(&item));
    assert!(decision.remember_text.is_none());
}

#[test]
fn paste_failure_keeps_text_for_copy_and_paste_last() {
    let decision = Recovery::decide(&attempt(Outcome::PasteFailed, "Szia, ez megvan.", "szia ez megvan"));
    let item = decision.item.expect("paste row");
    assert_eq!(item.status, HistoryStatus::PasteFailed);
    assert_eq!(item.text, "Szia, ez megvan.");
    assert_eq!(item.reason, "A szöveg megvan, a beillesztés nem sikerült.");
    assert!(item.audio_name.is_none());
    assert!(!decision.keep_audio);
    assert_eq!(decision.remember_text.as_deref(), Some("Szia, ez megvan."));
    let reused = Recovery::reusable_text(decision.remember_text.as_deref(), &[]);
    assert_eq!(reused.as_deref(), Some("Szia, ez megvan."));
}

#[test]
fn later_empty_failure_does_not_hide_the_last_transcript() {
    let pasted = Recovery::decide(&attempt(Outcome::PasteFailed, "Szia, ez megvan.", "szia"));
    let err = FlowError::Network;
    let failed = Recovery::decide(&attempt(Outcome::Failed(&err), "", ""));
    assert!(failed.remember_text.is_none());
    let history = vec![pasted.item.unwrap(), failed.item.unwrap()];
    let reused = Recovery::reusable_text(pasted.remember_text.as_deref(), &history);
    assert_eq!(reused.as_deref(), Some("Szia, ez megvan."));
    let fallback = Recovery::reusable_text(None, &history);
    assert_eq!(fallback.as_deref(), Some("Szia, ez megvan."));
}

#[test]
fn accidental_tap_is_not_saved() {
    let mut input = attempt(Outcome::NoSpeech, "", "");
    input.duration_seconds = 0.4;
    input.had_speech = false;
    input.has_wav = true;
    let decision = Recovery::decide(&input);
    assert!(decision.item.is_none());
    assert!(!decision.keep_audio);
    assert!(decision.discarded);
    assert!(decision.remember_text.is_none());
    assert!(decision.log_line.is_none());
}

#[test]
fn short_real_speech_is_still_saved_when_transcription_fails() {
    let err = FlowError::Server;
    let mut input = attempt(Outcome::Failed(&err), "", "");
    input.duration_seconds = 0.3;
    input.had_speech = true;
    let decision = Recovery::decide(&input);
    assert_eq!(decision.item.unwrap().status, HistoryStatus::Failed);
    assert!(decision.keep_audio);
}

#[test]
fn history_off_or_never_still_remembers_text_but_drops_audio() {
    let mut off = attempt(Outcome::PasteFailed, "Szia.", "szia");
    off.history_enabled = false;
    let decision = Recovery::decide(&off);
    assert!(decision.item.is_none());
    assert!(!decision.keep_audio);
    assert_eq!(decision.remember_text.as_deref(), Some("Szia."));

    let err = FlowError::Network;
    let mut never = attempt(Outcome::Failed(&err), "", "");
    never.retention = HistoryRetention::Never;
    let decision = Recovery::decide(&never);
    assert!(decision.item.is_none());
    assert!(!decision.keep_audio);
    assert!(decision.reason.contains("A kapcsolat megszakadt."));
    assert!(!decision.reason.contains("gépen"));
}

#[test]
fn retry_reads_saved_audio_and_success_clears_it() {
    let err = FlowError::Timeout;
    let decision = Recovery::decide(&attempt(Outcome::Failed(&err), "", ""));
    let mut item = decision.item.unwrap();
    let dir = temp_audio();
    let name = item.audio_name.clone().unwrap();
    let wav = b"RIFF-local-audio-not-a-secret";
    AudioFolder::write(&dir, &name, wav).unwrap();
    let loaded = Recovery::load_for_retry(&dir, &item).unwrap();
    assert_eq!(loaded, wav);

    let update = Recovery::settle_retry(&mut item, "holnap", "Holnap reggel indulunk.", Outcome::Success);
    assert_eq!(item.status, HistoryStatus::Success);
    assert!(item.audio_name.is_none());
    assert_eq!(item.text, "Holnap reggel indulunk.");
    assert_eq!(update.delete_audio.as_deref(), Some(name.as_str()));
    assert_eq!(update.remember_text.as_deref(), Some("Holnap reggel indulunk."));
    AudioFolder::remove(&dir, &update.delete_audio.unwrap()).unwrap();
    assert!(!Recovery::retry_allowed(&item));
    assert!(Recovery::load_for_retry(&dir, &item).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retry_failure_keeps_the_same_audio_file() {
    let err = FlowError::Network;
    let decision = Recovery::decide(&attempt(Outcome::Failed(&err), "", ""));
    let mut item = decision.item.unwrap();
    let name = item.audio_name.clone().unwrap();
    let again = FlowError::Server;
    let update = Recovery::settle_retry(&mut item, "", "", Outcome::Failed(&again));
    assert_eq!(item.status, HistoryStatus::Failed);
    assert_eq!(item.audio_name.as_deref(), Some(name.as_str()));
    assert!(update.delete_audio.is_none());
    assert!(Recovery::retry_allowed(&item));
    assert!(!update.log_line.contains(KEY));
}

#[test]
fn retry_paste_failure_keeps_text_and_drops_audio() {
    let err = FlowError::Network;
    let decision = Recovery::decide(&attempt(Outcome::Failed(&err), "", ""));
    let mut item = decision.item.unwrap();
    let name = item.audio_name.clone().unwrap();
    let update = Recovery::settle_retry(&mut item, "szia", "Szia.", Outcome::PasteFailed);
    assert_eq!(item.status, HistoryStatus::PasteFailed);
    assert_eq!(item.text, "Szia.");
    assert!(item.audio_name.is_none());
    assert_eq!(update.delete_audio.as_deref(), Some(name.as_str()));
    assert_eq!(Recovery::reusable_text(update.remember_text.as_deref(), &[item]).as_deref(), Some("Szia."));
}

#[test]
fn secrets_never_land_in_history_or_logs() {
    let err = FlowError::BadRequest(KEY.to_string());
    let leaked = format!("a kulcs {KEY} és {GEMINI} api_key={KEY} Bearer {KEY}");
    let mut input = attempt(Outcome::Failed(&err), &leaked, &leaked);
    input.app_name = KEY;
    let decision = Recovery::decide(&input);
    let item = decision.item.expect("row");
    let blob = format!("{} {} {} {} {}", item.text, item.raw_text, item.app_name, item.reason, decision.log_line.unwrap());
    assert!(!blob.contains("sk-"));
    assert!(!blob.contains("AIza"));
    assert!(!blob.contains("Bearer"));
    assert!(blob.contains("[rejtett]"));
    let json = serde_json::to_string(&item).unwrap();
    assert!(!json.contains("sk-"));
    assert!(!json.contains("AIza"));
    assert!(!item.audio_name.unwrap_or_default().contains("sk-"));
}

#[test]
fn redaction_leaves_ordinary_words_alone() {
    assert_eq!(Recovery::redact_secrets("ask-abcdefghijk holnap"), "ask-abcdefghijk holnap");
    assert_eq!(Recovery::redact_secrets(&format!("kulcs {KEY} vége")), "kulcs [rejtett] vége");
}

#[test]
fn unsafe_audio_names_are_rejected() {
    let dir = temp_audio();
    assert!(AudioFolder::write(&dir, "../x.wav", b"no").is_err());
    assert!(AudioFolder::write(&dir, "sk-abcdefghij.wav", b"no").is_err());
    assert!(AudioFolder::write(&dir, "notes.txt", b"no").is_err());
    let name = AudioFolder::name_for(uuid::Uuid::new_v4());
    AudioFolder::write(&dir, &name, b"RIFF").unwrap();
    std::fs::write(dir.join("notes.txt"), b"keep").unwrap();
    let mut keep = HashSet::new();
    keep.insert(name);
    AudioFolder::sweep(&dir, &keep).unwrap();
    assert_eq!(std::fs::read(dir.join("notes.txt")).unwrap(), b"keep");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retention_drops_expired_audio_names() {
    let now = 1_700_000_000_000i64;
    let mut recent = HistoryItem::new("friss".into(), "friss".into(), "A".into(), 2.0);
    recent.date_ms = now;
    recent.status = HistoryStatus::Failed;
    recent.audio_name = Some(AudioFolder::name_for(recent.id));
    let mut old = HistoryItem::new(String::new(), String::new(), "A".into(), 3.0);
    old.date_ms = now - 40 * 86_400_000;
    old.status = HistoryStatus::Interrupted;
    old.reason = "A diktálás megszakadt.".into();
    old.audio_name = Some(AudioFolder::name_for(old.id));
    let old_audio = old.audio_name.clone().unwrap();
    let (kept, drop) = HistoryLogic::retain(&[recent.clone(), old.clone()], HistoryRetention::Days30, now, 400);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].text, "friss");
    assert_eq!(drop, vec![old_audio]);
    let (none, _drop) = HistoryLogic::retain(&[recent.clone()], HistoryRetention::Never, now, 400);
    assert!(none.is_empty());
    assert_eq!(HistorySearch::filter(&[old], "megszakadt").len(), 1);
}

#[test]
fn old_history_json_still_loads_as_success() {
    let old = r#"{"id":"00000000-0000-0000-0000-000000000000","dateMs":1,"text":"Szia","rawText":"szia","appName":"A","durationSeconds":1.0,"kind":"dictation"}"#;
    let item: HistoryItem = serde_json::from_str(old).unwrap();
    assert_eq!(item.status, HistoryStatus::Success);
    assert!(item.reason.is_empty());
    assert!(item.audio_name.is_none());
    assert!(item.has_text());
}
