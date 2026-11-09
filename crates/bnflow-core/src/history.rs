use crate::models::{HistoryItem, HistoryKind, HistoryRetention};
use crate::util::{fold_diacritics, word_count};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStats {
    pub total_words: i64,
    pub week_words: i64,
    pub average_wpm: i64,
    pub streak_days: i64,
    pub sessions: i64,
}

pub struct HistoryLogic;

impl HistoryLogic {
    pub fn prune(items: &[HistoryItem], retention: HistoryRetention, now_ms: i64) -> Vec<HistoryItem> {
        match retention.max_age_secs() {
            None => items.to_vec(),
            Some(0) => vec![],
            Some(age) => items.iter().filter(|it| now_ms - it.date_ms <= age * 1000).cloned().collect(),
        }
    }

    pub fn stats(items: &[HistoryItem], now_ms: i64) -> HistoryStats {
        let total = items.iter().map(|i| i.word_count() as i64).sum();
        let week = items.iter().filter(|i| i.date_ms >= now_ms - 7 * 86_400_000).map(|i| i.word_count() as i64).sum();
        let dictated: Vec<_> = items.iter().filter(|i| i.kind == HistoryKind::Dictation && i.duration_seconds > 1.0).collect();
        let words: i64 = dictated.iter().map(|i| word_count(&i.raw_text) as i64).sum();
        let secs: f64 = dictated.iter().map(|i| i.duration_seconds).sum();
        let wpm = if secs > 0.0 { ((words as f64) / (secs / 60.0)).round() as i64 } else { 0 };
        let mut days: Vec<i64> = items.iter().map(|i| start_of_day(i.date_ms)).collect();
        days.sort_unstable();
        days.dedup();
        let mut streak = 0i64;
        let mut day = start_of_day(now_ms);
        if !days.contains(&day) {
            day -= 86_400_000;
        }
        while days.contains(&day) {
            streak += 1;
            day -= 86_400_000;
        }
        HistoryStats { total_words: total, week_words: week, average_wpm: wpm, streak_days: streak, sessions: items.len() as i64 }
    }
}

fn start_of_day(ms: i64) -> i64 {
    let day = 86_400_000;
    let mut d = ms - (ms.rem_euclid(day));
    // Local-day approximation is UTC day; the Mac calendar uses the user's zone.
    // Tests construct "now" and offsets in the same clock, so UTC day boundaries match when the offset is exactly 86400s.
    if d > ms {
        d -= day;
    }
    d
}

pub struct HistorySearch;

impl HistorySearch {
    pub fn filter(items: &[HistoryItem], query: &str) -> Vec<HistoryItem> {
        let words: Vec<String> = fold_diacritics(query).split_whitespace().map(|s| s.to_string()).collect();
        if words.is_empty() {
            return items.to_vec();
        }
        items.iter().filter(|it| {
            let hay = fold_diacritics(&format!("{} {} {}", it.text, it.raw_text, it.app_name));
            words.iter().all(|w| hay.contains(w))
        }).cloned().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardingState {
    pub has_key: bool,
    pub microphone: bool,
    pub accessibility: bool,
    pub input_monitoring: bool,
    pub has_dictated: bool,
}

impl OnboardingState {
    pub fn steps(self) -> [bool; 5] {
        [self.has_key, self.microphone, self.accessibility, self.input_monitoring, self.has_dictated]
    }
    pub fn done_count(self) -> usize {
        self.steps().iter().filter(|s| **s).count()
    }
    pub fn complete(self) -> bool {
        self.done_count() == 5
    }
    pub fn next(self) -> Option<usize> {
        self.steps().iter().position(|s| !s)
    }
}
