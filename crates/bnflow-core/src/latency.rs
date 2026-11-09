use crate::audio::{analyze, encode_wav, float_to_i16, prepare, AudioStats};

pub struct AudioFingerprint;

impl AudioFingerprint {
    pub fn hash(data: &[u8]) -> u64 {
        let mut h: u64 = 14695981039346656037;
        for b in data {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptRequestId {
    pub audio: u64,
    pub prompt: u64,
}

impl TranscriptRequestId {
    pub fn prompt_hash(provider: &str, model: &str, fallback: &str, language: &str, terms: &[String]) -> u64 {
        let mut s = format!("{provider}\n{model}\n{fallback}\n{language}");
        for t in terms {
            s.push('\n');
            s.push_str(t);
        }
        AudioFingerprint::hash(s.as_bytes())
    }
    pub fn make(wav: &[u8], provider: &str, model: &str, fallback: &str, language: &str, terms: &[String]) -> Self {
        Self { audio: AudioFingerprint::hash(wav), prompt: Self::prompt_hash(provider, model, fallback, language, terms) }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTurn {
    pub wav: Vec<u8>,
    pub stats: AudioStats,
    pub fingerprint: u64,
    pub trailing_silence: f64,
}

impl PreparedTurn {
    pub fn make(raw: &[f32], sample_rate: i32) -> Self {
        let stats = analyze(raw, sample_rate);
        let prepared = prepare(raw, sample_rate);
        let wav = encode_wav(&float_to_i16(&prepared), sample_rate as u32);
        let trail = if let Some((_, hi)) = stats.speech_range {
            (raw.len().saturating_sub(hi)) as f64 / sample_rate.max(1) as f64
        } else {
            stats.seconds
        };
        let fingerprint = AudioFingerprint::hash(&wav);
        Self { wav, stats, fingerprint, trailing_silence: trail }
    }
}

pub struct SpeculationPolicy;

impl SpeculationPolicy {
    pub const TRAILING_SILENCE: f64 = 0.45;
    pub const MINIMUM_SPEECH: f64 = 0.35;
    pub const MAX_STARTS: i32 = 2;

    pub fn should_start(stats: &AudioStats, trail: f64, starts: i32, inflight: Option<TranscriptRequestId>, candidate: TranscriptRequestId, enabled: bool, recording: bool, closed: bool, mic_inactive: bool) -> bool {
        if !enabled || !recording || closed || mic_inactive {
            return false;
        }
        if starts >= Self::MAX_STARTS {
            return false;
        }
        if inflight == Some(candidate) {
            return false;
        }
        if crate::audio::is_silent(stats) {
            return false;
        }
        if stats.speech_seconds < Self::MINIMUM_SPEECH {
            return false;
        }
        if trail < Self::TRAILING_SILENCE {
            return false;
        }
        true
    }
}

pub struct CallBudget;

impl CallBudget {
    pub const BASELINE_TRANSCRIPTIONS: i32 = 1;
    pub const BASELINE_CLEANUPS: i32 = 1;

    pub fn per_take(prefetch_enabled: bool, abandoned_starts: i32) -> (i32, i32) {
        let extra = if prefetch_enabled { SpeculationPolicy::MAX_STARTS.min(abandoned_starts.max(0)) } else { 0 };
        (Self::BASELINE_TRANSCRIPTIONS + extra, Self::BASELINE_CLEANUPS + extra)
    }

    pub fn worst_case(prefetch_enabled: bool) -> (i32, i32) {
        Self::per_take(prefetch_enabled, if prefetch_enabled { SpeculationPolicy::MAX_STARTS } else { 0 })
    }
}

pub struct TurnSelection;

impl TurnSelection {
    pub fn use_inflight(inflight: Option<TranscriptRequestId>, final_id: TranscriptRequestId) -> bool {
        inflight == Some(final_id)
    }
}

pub struct PasteConfirm;

impl PasteConfirm {
    pub const FIRST_WAIT_MS: i32 = 40;
    pub const POLL_MS: i32 = 40;
    pub const MAX_POLLS: i32 = 8;

    pub fn should_poll(polls_done: i32, value_changed: bool, can_read: bool) -> bool {
        can_read && !value_changed && polls_done < Self::MAX_POLLS
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationTrace {
    pub finalize_ms: i32,
    pub transcribe_wait_ms: i32,
    pub overlapped_ms: i32,
    pub cleanup_ms: i32,
    pub paste_ms: i32,
    pub post_speech_ms: i32,
    pub speculated: bool,
    pub bytes: i32,
}

impl DictationTrace {
    pub fn log_line(&self) -> String {
        format!(
            "lat finalize={}ms transcribeWait={}ms overlapped={}ms cleanup={}ms paste={}ms postSpeech={}ms bytes={} spec={}",
            self.finalize_ms, self.transcribe_wait_ms, self.overlapped_ms, self.cleanup_ms, self.paste_ms, self.post_speech_ms, self.bytes, if self.speculated { 1 } else { 0 }
        )
    }
}
