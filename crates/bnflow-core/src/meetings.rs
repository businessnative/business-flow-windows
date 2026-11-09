use chrono::{Datelike, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use chrono_tz::Tz;
use regex::Regex;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingPlatform {
    GoogleMeet,
    Teams,
}

impl MeetingPlatform {
    pub fn title(self) -> &'static str {
        match self {
            Self::GoogleMeet => "Google Meet",
            Self::Teams => "Microsoft Teams",
        }
    }
    pub fn join_supported(self) -> bool {
        self == Self::GoogleMeet
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingEvent {
    pub uid: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub url: String,
    pub platform: MeetingPlatform,
}

impl MeetingEvent {
    pub fn id(&self) -> String {
        format!("{}@{}", self.uid, self.start)
    }
}

pub struct MeetingLinks;

impl MeetingLinks {
    pub fn find(text: &str) -> Option<(String, MeetingPlatform)> {
        let t = text.replace("\\n", " ").replace("\\,", ",");
        let patterns = [
            (Regex::new(r"https://meet\.google\.com/[a-z]{3}-[a-z]{4}-[a-z]{3}").unwrap(), MeetingPlatform::GoogleMeet),
            (Regex::new(r#"https://teams\.(microsoft|live)\.com/(l/meetup-join|meet)/[^\s"<>]+"#).unwrap(), MeetingPlatform::Teams),
        ];
        let mut best: Option<(usize, String, MeetingPlatform)> = None;
        for (re, plat) in patterns {
            if let Some(m) = re.find(&t) {
                if best.as_ref().map(|b| m.start() < b.0).unwrap_or(true) {
                    best = Some((m.start(), m.as_str().to_string(), plat));
                }
            }
        }
        best.map(|(_, u, p)| (u, p))
    }
}

struct Prop {
    name: String,
    params: HashMap<String, String>,
    value: String,
}

pub struct Ics;

impl Ics {
    pub fn meetings(text: &str, from: i64, to: i64, default_tz: Tz) -> Vec<MeetingEvent> {
        let props = unfold(text);
        let mut raws: Vec<HashMap<String, Vec<Prop>>> = Vec::new();
        let mut cur: Option<HashMap<String, Vec<Prop>>> = None;
        let mut depth = 0i32;
        for pr in props {
            if pr.name == "BEGIN" && pr.value == "VEVENT" {
                cur = Some(HashMap::new());
                depth = 0;
                continue;
            }
            if pr.name == "BEGIN" && cur.is_some() {
                depth += 1;
                continue;
            }
            if pr.name == "END" && cur.is_some() && depth > 0 {
                depth -= 1;
                continue;
            }
            if pr.name == "END" && pr.value == "VEVENT" {
                if let Some(c) = cur.take() {
                    raws.push(c);
                }
                continue;
            }
            if cur.is_some() && depth == 0 {
                cur.as_mut().unwrap().entry(pr.name.clone()).or_default().push(pr);
            }
        }
        let mut overridden = HashSet::new();
        for r in &raws {
            if let (Some(uid), Some(rid)) = (first(r, "UID"), first(r, "RECURRENCE-ID")) {
                if let Some((d, _)) = parse_date(&rid.value, rid.params.get("TZID").map(|s| s.as_str()), default_tz) {
                    overridden.insert(format!("{}@{d}", uid.value));
                }
            }
        }
        let mut out = Vec::new();
        for r in &raws {
            let Some(uid) = first(r, "UID").map(|p| p.value.clone()) else { continue };
            let Some(ds) = first(r, "DTSTART") else { continue };
            let Some((start, all_day)) = parse_date(&ds.value, ds.params.get("TZID").map(|s| s.as_str()), default_tz) else { continue };
            if all_day {
                continue;
            }
            if first(r, "STATUS").map(|p| p.value.eq_ignore_ascii_case("CANCELLED")).unwrap_or(false) {
                continue;
            }
            let mut end = start + 3600;
            if let Some(de) = first(r, "DTEND") {
                if let Some((e, _)) = parse_date(&de.value, de.params.get("TZID").map(|s| s.as_str()), default_tz) {
                    end = e;
                }
            }
            let blob = ["X-GOOGLE-CONFERENCE", "URL", "LOCATION", "DESCRIPTION"].iter().filter_map(|n| first(r, n).map(|p| unescape(&p.value))).collect::<Vec<_>>().join(" ");
            let Some((url, plat)) = MeetingLinks::find(&blob) else { continue };
            let title = unescape(&first(r, "SUMMARY").map(|p| p.value.clone()).unwrap_or_else(|| "Meeting".into()));
            let dur = end - start;
            let mut starts = vec![start];
            if let Some(rr) = first(r, "RRULE") {
                if first(r, "RECURRENCE-ID").is_none() {
                    let ex: HashSet<i64> = r.get("EXDATE").into_iter().flatten().flat_map(|p| {
                        p.value.split(',').filter_map(|v| parse_date(v, p.params.get("TZID").map(|s| s.as_str()), default_tz).map(|d| d.0)).collect::<Vec<_>>()
                    }).collect();
                    let tz = ds.params.get("TZID").and_then(|s| s.parse::<Tz>().ok()).unwrap_or(default_tz);
                    starts = expand(&rr.value, start, to, tz).into_iter().filter(|s| !ex.contains(s)).filter(|s| !overridden.contains(&format!("{uid}@{s}"))).collect();
                }
            }
            for s in starts {
                if s < to && s + dur > from {
                    out.push(MeetingEvent { uid: uid.clone(), title: title.clone(), start: s, end: s + dur, url: url.clone(), platform: plat });
                }
            }
        }
        out.sort_by_key(|e| e.start);
        out
    }
}

fn first<'a>(map: &'a HashMap<String, Vec<Prop>>, name: &str) -> Option<&'a Prop> {
    map.get(name).and_then(|v| v.first())
}

fn unfold(text: &str) -> Vec<Prop> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.replace("\r\n", "\n").split('\n') {
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !lines.is_empty() {
            let last = lines.last_mut().unwrap();
            last.push_str(&raw[1..]);
        } else {
            lines.push(raw.to_string());
        }
    }
    lines.into_iter().filter_map(|l| {
        let colon = value_colon(&l)?;
        let head = &l[..colon];
        let value = l[colon + 1..].to_string();
        let parts: Vec<&str> = head.split(';').collect();
        let mut params = HashMap::new();
        for p in parts.iter().skip(1) {
            if let Some((k, v)) = p.split_once('=') {
                params.insert(k.to_uppercase(), v.trim_matches('"').to_string());
            }
        }
        Some(Prop { name: parts.first().unwrap_or(&"").to_uppercase(), params, value })
    }).collect()
}

fn value_colon(l: &str) -> Option<usize> {
    let mut q = false;
    for (i, c) in l.char_indices() {
        if c == '"' {
            q = !q;
        } else if c == ':' && !q {
            return Some(i);
        }
    }
    None
}

fn parse_date(v: &str, tzid: Option<&str>, default_tz: Tz) -> Option<(i64, bool)> {
    if v.len() == 8 {
        let date = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        let naive = date.and_hms_opt(0, 0, 0)?;
        let dt = default_tz.from_local_datetime(&naive).single()?;
        return Some((dt.timestamp(), true));
    }
    if let Some(stripped) = v.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S").ok()?;
        return Some((naive.and_utc().timestamp(), false));
    }
    let naive = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    let tz = tzid.and_then(|s| s.parse::<Tz>().ok()).unwrap_or(default_tz);
    Some((tz.from_local_datetime(&naive).single()?.timestamp(), false))
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

fn expand(rule: &str, start: i64, horizon: i64, tz: Tz) -> Vec<i64> {
    let mut kv = HashMap::new();
    for part in rule.split(';') {
        if let Some((k, v)) = part.split_once('=') {
            kv.insert(k.to_uppercase(), v.to_string());
        }
    }
    let interval = kv.get("INTERVAL").and_then(|s| s.parse::<i64>().ok()).unwrap_or(1).max(1);
    let count = kv.get("COUNT").and_then(|s| s.parse::<i32>().ok());
    let mut until = horizon;
    if let Some(u) = kv.get("UNTIL") {
        if let Some((d, all_day)) = parse_date(u, None, tz) {
            until = horizon.min(if all_day { d + 86_399 } else { d });
        }
    }
    let day_map = [("SU", 1), ("MO", 2), ("TU", 3), ("WE", 4), ("TH", 5), ("FR", 6), ("SA", 7)];
    let by_day: Vec<i32> = kv.get("BYDAY").map(|s| s.split(',').filter_map(|d| day_map.iter().find(|x| x.0 == &d[d.len().saturating_sub(2)..]).map(|x| x.1)).collect()).unwrap_or_default();
    let start_dt = tz.timestamp_opt(start, 0).single().unwrap();
    let mut out = Vec::new();
    let mut n = 0i32;
    let mut emit = |d: i64| -> bool {
        if d > until {
            return false;
        }
        if let Some(c) = count {
            if n >= c {
                return false;
            }
        }
        if d >= start {
            out.push(d);
            n += 1;
            if out.len() > 2000 {
                return false;
            }
        }
        true
    };
    match kv.get("FREQ").map(|s| s.as_str()) {
        Some("DAILY") => {
            let mut d = start;
            while emit(d) {
                d = add_days(d, interval, tz);
            }
        }
        Some("WEEKLY") => {
            let days = if by_day.is_empty() { vec![swift_weekday(start_dt)] } else { let mut d = by_day.clone(); d.sort(); d };
            let wd = swift_weekday(start_dt);
            let mut week_start = add_days(start, -(wd as i64 - 1), tz);
            'outer: loop {
                for day in &days {
                    let d = add_days(week_start, (*day as i64) - 1, tz);
                    if d < start {
                        continue;
                    }
                    if !emit(d) {
                        break 'outer;
                    }
                }
                week_start = add_days(week_start, 7 * interval, tz);
                if week_start > until {
                    break;
                }
            }
        }
        Some("MONTHLY") => {
            let mut i = 0i64;
            while let Some(d) = add_months(start, i * interval, tz) {
                if !emit(d) {
                    break;
                }
                i += 1;
                if i > 600 {
                    break;
                }
            }
        }
        _ => out = vec![start],
    }
    out
}

fn swift_weekday(dt: chrono::DateTime<Tz>) -> i32 {
    match dt.weekday() {
        chrono::Weekday::Sun => 1,
        chrono::Weekday::Mon => 2,
        chrono::Weekday::Tue => 3,
        chrono::Weekday::Wed => 4,
        chrono::Weekday::Thu => 5,
        chrono::Weekday::Fri => 6,
        chrono::Weekday::Sat => 7,
    }
}

fn add_days(ts: i64, days: i64, tz: Tz) -> i64 {
    let dt = tz.timestamp_opt(ts, 0).single().unwrap();
    let date = dt.date_naive().checked_add_signed(chrono::Duration::days(days)).unwrap_or(dt.date_naive());
    let naive = date.and_hms_opt(dt.hour(), dt.minute(), dt.second()).unwrap();
    tz.from_local_datetime(&naive).single().map(|d| d.timestamp()).unwrap_or(ts + days * 86400)
}

fn add_months(ts: i64, months: i64, tz: Tz) -> Option<i64> {
    let dt = tz.timestamp_opt(ts, 0).single()?;
    let mut month = dt.month() as i32 - 1 + months as i32;
    let mut year = dt.year();
    year += month.div_euclid(12);
    month = month.rem_euclid(12);
    let date = NaiveDate::from_ymd_opt(year, (month + 1) as u32, dt.day())?;
    let naive = date.and_hms_opt(dt.hour(), dt.minute(), dt.second())?;
    tz.from_local_datetime(&naive).single().map(|d| d.timestamp())
}

pub struct MeetingNotes;

impl MeetingNotes {
    pub fn slug(event: &MeetingEvent, tz: Tz) -> String {
        let dt = tz.timestamp_opt(event.start, 0).single().unwrap();
        let stamp = format!("{:04}-{:02}-{:02} {:02}{:02}", dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute());
        let safe: String = event.title.chars().map(|c| if "/\\:*?\"<>|\n\r\t".contains(c) { ' ' } else { c }).collect();
        let safe = safe.trim();
        let safe: String = safe.chars().take(60).collect();
        format!("{stamp} – {}", if safe.is_empty() { "Meeting" } else { safe.as_str() })
    }

    pub fn summary_input(title: &str, transcript: &str, limit: usize) -> String {
        let body = if transcript.chars().count() <= limit {
            transcript.to_string()
        } else {
            let chars: Vec<char> = transcript.chars().collect();
            let head: String = chars.iter().take(limit * 2 / 3).collect();
            let tail: String = chars.iter().skip(chars.len().saturating_sub(limit / 3)).collect();
            format!("{head}\n\n[…az átirat közepe a hossz miatt kimaradt…]\n\n{tail}")
        };
        format!("Meeting címe: {title}\n\nÁtirat:\n{body}")
    }

    pub const SUMMARY_SYSTEM: &'static str = "Meeting-jegyzetelő vagy. A felhasználó egy meeting gépi átiratát adja meg (több résztvevő, beszélőnév nélkül, lehetnek félrehallások).\nÍrj tömör, pontos magyar összefoglalót Markdownban, csak az átiratban ténylegesen elhangzottak alapján; ne találj ki neveket, számokat vagy döntéseket.\nSzerkezet:\n## Összefoglaló\n(3–6 mondat)\n## Fő témák\n- …\n## Döntések\n- … (ha nincs: „Nem hangzott el egyértelmű döntés.”)\n## Teendők\n- [ ] Ki – mit – határidő (a felelős neve a feliratból, ha van) (ami elhangzott; ha nincs felelős: „(nincs megnevezve)”)\n## Nyitott kérdések\n- …\nHa az átirat angol vagy más nyelvű, az összefoglaló akkor is magyar legyen, a szakkifejezéseket hagyd eredetiben.";
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeetingCaption {
    pub speaker: String,
    pub text: String,
    pub t: f64,
    pub t_end: Option<f64>,
}

pub struct MeetingCaptions;

impl MeetingCaptions {
    pub fn parse(jsonl: &str) -> Vec<MeetingCaption> {
        let mut out = Vec::new();
        for line in jsonl.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let speaker = v["speaker"].as_str().unwrap_or("").to_string();
            let text = v["text"].as_str().unwrap_or("").trim().to_string();
            let t = v["t"].as_f64().unwrap_or(0.0);
            let t_end = v["tEnd"].as_f64();
            if text.is_empty() || speaker.is_empty() {
                continue;
            }
            if let Some(last) = out.last_mut() {
                let last: &mut MeetingCaption = last;
                if last.speaker == speaker && t - last.t_end.unwrap_or(last.t) < 20.0 {
                    if text.starts_with(&last.text) {
                        last.text = text;
                    } else if !last.text.ends_with(&text) {
                        last.text.push(' ');
                        last.text.push_str(&text);
                    }
                    last.t_end = t_end.or(Some(t));
                    continue;
                }
            }
            out.push(MeetingCaption { speaker, text, t, t_end });
        }
        out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    pub fn stamp(t: f64) -> String {
        let s = t as i64;
        format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }

    pub fn markdown(caps: &[MeetingCaption]) -> String {
        caps.iter().map(|c| format!("**{}** [{}]: {}", c.speaker, Self::stamp(c.t), c.text)).collect::<Vec<_>>().join("\n\n")
    }

    pub fn talk_time(caps: &[MeetingCaption]) -> Vec<(String, f64)> {
        let mut d: HashMap<String, f64> = HashMap::new();
        for c in caps {
            *d.entry(c.speaker.clone()).or_default() += ((c.t_end.unwrap_or(c.t)) - c.t).max(1.0);
        }
        let mut v: Vec<_> = d.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v
    }
}

pub struct MeetingAudio;

impl MeetingAudio {
    pub fn chunks(pcm: &[u8], seconds: i64) -> Vec<Vec<u8>> {
        let bytes_per = seconds as usize * 16_000 * 2;
        let mut out = Vec::new();
        let mut i = 0;
        while i < pcm.len() {
            let end = pcm.len().min(i + bytes_per);
            let len = end - i;
            let trimmed = end - (len % 2);
            out.push(pcm[i..trimmed].to_vec());
            i = end;
        }
        out
    }

    pub fn is_silent(pcm: &[u8]) -> bool {
        let samples = Self::samples(pcm);
        let st = crate::audio::analyze(&samples, 16_000);
        st.peak < 0.01 || st.speech_seconds < 1.0
    }

    pub fn samples(pcm: &[u8]) -> Vec<f32> {
        pcm.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect()
    }

    pub fn wav_len(pcm_len: usize) -> usize {
        44 + pcm_len - (pcm_len % 2)
    }
}
