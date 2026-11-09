use crate::http_client::Net;
use crate::secrets;
use async_trait::async_trait;
use bnflow_core::{AIRouter, AppSettings, ChatProvider, FlowError, Ics, MeetingAudio, MeetingCaptions, MeetingEvent, MeetingNotes, MeetingPlatform, ProviderKeys};
use chrono::Utc;
use chrono_tz::UTC;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Upcoming {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub url: String,
    pub platform: String,
    pub join_supported: bool,
    pub skipped: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCard {
    pub id: String,
    pub title: String,
    pub status: String,
    pub has_summary: bool,
}

pub fn notes_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BFLOW_NOTES_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::document_dir().unwrap_or_else(|| PathBuf::from(".")).join("Business Flow").join("Meeting-jegyzetek")
}

pub fn list_notes() -> Vec<NoteCard> {
    let dir = notes_dir();
    let Ok(read) = std::fs::read_dir(&dir) else { return vec![] };
    let mut out = Vec::new();
    for entry in read.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta_path = entry.path().join("meta.json");
        let title = std::fs::read_to_string(&meta_path).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v["title"].as_str().map(|s| s.to_string())).unwrap_or(name.clone());
        let status = std::fs::read_to_string(&meta_path).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v["status"].as_str().map(|s| s.to_string())).unwrap_or_else(|| "unknown".into());
        out.push(NoteCard { id: name, title, status, has_summary: entry.path().join("summary.md").exists() });
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

pub async fn upcoming(settings: &AppSettings) -> Result<Vec<Upcoming>, String> {
    let Some(url) = secrets::read_named("ical-url") else { return Ok(vec![]) };
    let body = fetch_text(&url).await?;
    let now = Utc::now().timestamp();
    let events = Ics::meetings(&body, now - 120, now + 14 * 86_400, UTC);
    Ok(events.into_iter().map(|e| Upcoming {
        skipped: settings.skipped_meetings.iter().any(|id| id == &e.id()),
        id: e.id(),
        title: e.title.clone(),
        start: e.start,
        url: e.url.clone(),
        platform: e.platform.title().into(),
        join_supported: e.platform.join_supported(),
    }).collect())
}

pub fn queue_join(url: &str, title: Option<&str>) -> Result<String, String> {
    if !url.starts_with("https://meet.google.com/") {
        return Err("Csak Google Meet link fogadható el. A Teams csatlakozás még nincs kész.".into());
    }
    let dir = notes_dir().join(".queue");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let path = dir.join(format!("{stamp}.json"));
    let body = serde_json::json!({ "url": url, "title": title, "requestedAt": Utc::now().to_rfc3339() });
    std::fs::write(path, body.to_string()).map_err(|e| e.to_string())?;
    Ok("Kérés sorba állítva. A jegyzetelő fél percen belül csatlakozik, ha a Meetingek funkció be van kapcsolva.".into())
}

pub fn spawn_loop(settings: Arc<Mutex<AppSettings>>) {
    tauri::async_runtime::spawn(async move {
        let active: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let settings_now = settings.lock().map(|s| s.clone()).unwrap_or_default();
            if !settings_now.meetings_enabled {
                continue;
            }
            if let Ok(mut child) = active.lock() {
                if let Some(proc) = child.as_mut() {
                    if proc.try_wait().ok().flatten().is_some() {
                        *child = None;
                    } else {
                        continue;
                    }
                }
            }
            let _ = drain_queue(&settings_now, &active).await;
            let _ = maybe_join_calendar(&settings_now, &active).await;
        }
    });
}

async fn maybe_join_calendar(settings: &AppSettings, active: &Arc<Mutex<Option<Child>>>) -> Result<(), String> {
    let events = upcoming(settings).await?;
    let now = Utc::now().timestamp();
    if let Some(event) = events.into_iter().find(|e| e.join_supported && !e.skipped && e.start - 60 <= now && now <= e.start + 120) {
        start_join(&event.url, &event.title, event.start, &settings.meeting_bot_name, active)?;
    }
    Ok(())
}

async fn drain_queue(settings: &AppSettings, active: &Arc<Mutex<Option<Child>>>) -> Result<(), String> {
    let dir = notes_dir().join(".queue");
    let Ok(read) = std::fs::read_dir(&dir) else { return Ok(()) };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let _ = std::fs::remove_file(&path);
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let Some(url) = v["url"].as_str() else { continue };
        let title = v["title"].as_str().unwrap_or("Meeting");
        let _ = start_join(url, title, Utc::now().timestamp(), &settings.meeting_bot_name, active);
        break;
    }
    Ok(())
}

fn start_join(url: &str, title: &str, start: i64, name: &str, active: &Arc<Mutex<Option<Child>>>) -> Result<(), String> {
    if active.lock().map(|g| g.is_some()).unwrap_or(true) {
        return Err("Már fut egy jegyzetelő.".into());
    }
    let helper = find_helper().ok_or("A meetings/helper/join.mjs nem található. Állítsd a BNFLOW_MEETINGS_DIR változót a meetings mappára, és futtasd ott az npm installt.")?;
    let folder = notes_dir().join(safe_folder(title, start));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let pcm = folder.join("audio.pcm");
    let mut cmd = Command::new(find_node());
    cmd.arg(&helper).arg("--url").arg(url).arg("--name").arg(name).arg("--out").arg(&pcm).arg("--until").arg((start + 3 * 3600).to_string());
    cmd.current_dir(helper.parent().and_then(|p| p.parent()).unwrap_or(Path::new(".")));
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    let child = cmd.spawn().map_err(|e| format!("A Node nem indult el ({e}). Telepítsd a Node.js-t és a Google Chrome-ot."))?;
    write_meta(&folder, title, url, start, "joining");
    let folder_bg = folder.clone();
    let settings_bg = name.to_string();
    let _ = settings_bg;
    if let Ok(mut slot) = active.lock() {
        *slot = Some(child);
    }
    let active_bg = active.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let done = active_bg.lock().ok().and_then(|mut g| g.as_mut().and_then(|c| c.try_wait().ok()).flatten()).is_some();
            if done {
                if let Ok(mut g) = active_bg.lock() {
                    *g = None;
                }
                let _ = std::fs::write(folder_bg.join("status.txt"), "ended");
                break;
            }
        }
    });
    let folder_for_process = folder;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        for _ in 0..240 {
            if folder_for_process.join("status.txt").exists() || !folder_for_process.join("audio.pcm").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
        // The watcher writes status when the helper exits. Wait until then.
        for _ in 0..480 {
            if folder_for_process.join("status.txt").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
        let _ = finalize_notes(&folder_for_process).await;
    });
    Ok(())
}

async fn finalize_notes(folder: &Path) -> Result<(), String> {
    let pcm_path = folder.join("audio.pcm");
    let pcm = std::fs::read(&pcm_path).unwrap_or_default();
    let captions = std::fs::read_to_string(folder.join("captions.jsonl")).unwrap_or_default();
    let caps = MeetingCaptions::parse(&captions);
    let mut segments = Vec::new();
    let mut seconds = 0.0;
    if !pcm.is_empty() {
        for chunk in MeetingAudio::chunks(&pcm, 600) {
            seconds += chunk.len() as f64 / 32_000.0;
            if MeetingAudio::is_silent(&chunk) {
                segments.push(String::new());
                continue;
            }
            let samples = MeetingAudio::samples(&chunk);
            let wav = bnflow_core::audio::encode_wav(&bnflow_core::audio::float_to_i16(&samples), 16_000);
            let settings = crate::store::load_settings();
            let keys = secrets::load_keys();
            let (result, _) = AIRouter::new(Net).transcribe(&settings, &keys, &wav, &[]).await;
            segments.push(result.unwrap_or_default());
        }
        let _ = std::fs::remove_file(&pcm_path);
    }
    let title = std::fs::read_to_string(folder.join("meta.json")).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v["title"].as_str().map(|s| s.to_string())).unwrap_or_else(|| "Meeting".into());
    let transcript = if segments.iter().any(|s| !s.is_empty()) {
        segments.iter().enumerate().filter(|(_, s)| !s.is_empty()).map(|(i, s)| format!("**[{:02}:{:02}]** {s}\n", i * 10, 0)).collect::<String>()
    } else {
        MeetingCaptions::markdown(&caps)
    };
    let _ = std::fs::write(folder.join("transcript.md"), format!("# {title} – átirat\n\n{transcript}\n"));
    let settings = crate::store::load_settings();
    let keys = secrets::load_keys();
    let summary = if keys.is_empty() {
        "Nincs szövegkulcs az összefoglalóhoz.".into()
    } else {
        let chat = NoteChat { settings, keys };
        let user = MeetingNotes::summary_input(&title, &transcript, 120_000);
        match chat.complete(MeetingNotes::SUMMARY_SYSTEM, &user, None).await {
            Ok(text) => text,
            Err(err) => err.message(),
        }
    };
    let _ = std::fs::write(folder.join("summary.md"), summary);
    write_meta(folder, &title, "", 0, "done");
    let _ = seconds;
    Ok(())
}

fn write_meta(folder: &Path, title: &str, url: &str, start: i64, status: &str) {
    let body = serde_json::json!({ "title": title, "url": url, "start": start, "status": status, "platform": "googleMeet" });
    let _ = std::fs::write(folder.join("meta.json"), serde_json::to_string_pretty(&body).unwrap_or_default());
}

fn safe_folder(title: &str, start: i64) -> String {
    let event = MeetingEvent { uid: "q".into(), title: title.into(), start, end: start + 3600, url: String::new(), platform: MeetingPlatform::GoogleMeet };
    MeetingNotes::slug(&event, UTC)
}

fn find_node() -> String {
    std::env::var("BNFLOW_NODE").unwrap_or_else(|_| "node".into())
}

fn find_helper() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(dir) = std::env::var("BNFLOW_MEETINGS_DIR") {
        candidates.push(PathBuf::from(dir).join("helper").join("join.mjs"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("meetings").join("helper").join("join.mjs"));
            candidates.push(dir.join("..").join("meetings").join("helper").join("join.mjs"));
        }
    }
    candidates.push(PathBuf::from("meetings/helper/join.mjs"));
    candidates.into_iter().find(|p| p.exists())
}

async fn fetch_text(url: &str) -> Result<String, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Az iCal cím http vagy https legyen.".into());
    }
    let response = reqwest::Client::new().get(url).timeout(Duration::from_secs(20)).send().await.map_err(|_| "A naptár letöltése nem sikerült.".to_string())?;
    response.text().await.map_err(|_| "A naptár üres vagy olvashatatlan.".into())
}

struct NoteChat {
    settings: AppSettings,
    keys: ProviderKeys,
}

#[async_trait]
impl ChatProvider for NoteChat {
    async fn complete(&self, system: &str, user: &str, prediction: Option<&str>) -> Result<String, FlowError> {
        AIRouter::new(Net).complete(&self.settings, &self.keys, system, user, prediction).await
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Ignored {
    url: String,
}
