#[derive(Debug, Clone, PartialEq)]
pub struct AudioStats {
    pub seconds: f64,
    pub peak: f32,
    pub speech_seconds: f64,
    pub noise_floor: f32,
    pub speech_range: Option<(usize, usize)>,
}

const FRAME_SECONDS: f64 = 0.02;

pub fn analyze(samples: &[f32], sample_rate: i32) -> AudioStats {
    let n = samples.len();
    let seconds = n as f64 / sample_rate.max(1) as f64;
    let peak = samples.iter().fold(0f32, |a, s| a.max(s.abs()));
    let frame = ((sample_rate as f64) * FRAME_SECONDS).max(1.0) as usize;
    let mut rms = Vec::new();
    let mut i = 0;
    while i < n {
        let e = n.min(i + frame);
        let mut acc = 0f32;
        for s in &samples[i..e] {
            acc += s * s;
        }
        rms.push((acc / (e - i) as f32).sqrt());
        i = e;
    }
    if rms.is_empty() {
        return AudioStats { seconds: 0.0, peak: 0.0, speech_seconds: 0.0, noise_floor: 0.0, speech_range: None };
    }
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let floor = sorted[(sorted.len() - 1).min(sorted.len() / 10)];
    let loud = sorted[(sorted.len() - 1).min(sorted.len() * 95 / 100)];
    let threshold = 0.0012f32.max((floor * 2.5).max(loud * 0.12).min(loud * 0.5));
    let stationary = loud < floor * 1.8 && loud < 0.05;
    let mut speech_frames = 0usize;
    let mut first = -1isize;
    let mut last = -1isize;
    for (k, r) in rms.iter().enumerate() {
        if *r > threshold && !stationary {
            speech_frames += 1;
            if first < 0 {
                first = k as isize;
            }
            last = k as isize;
        }
    }
    let range = if first >= 0 {
        Some((first as usize * frame, n.min((last as usize + 1) * frame)))
    } else {
        None
    };
    AudioStats { seconds, peak, speech_seconds: speech_frames as f64 * FRAME_SECONDS, noise_floor: floor, speech_range: range }
}

pub fn high_pass(x: &[f32], sample_rate: i32, cutoff: f64) -> Vec<f32> {
    if x.len() <= 1 {
        return x.to_vec();
    }
    let rc = 1.0 / (2.0 * std::f64::consts::PI * cutoff);
    let dt = 1.0 / sample_rate as f64;
    let a = (rc / (rc + dt)) as f32;
    let mut out = vec![0f32; x.len()];
    let mut prev_x = x[0];
    let mut prev_y = 0f32;
    for i in 0..x.len() {
        let y = a * (prev_y + x[i] - prev_x);
        out[i] = y;
        prev_y = y;
        prev_x = x[i];
    }
    out
}

pub fn is_whisper(st: &AudioStats) -> bool {
    st.speech_seconds >= 0.3 && st.peak < 0.08 && st.peak > 0.002f32.max(st.noise_floor * 6.0)
}

pub fn gain(peak: f32, noise_floor: f32) -> f32 {
    if !(peak > 0.0005 && peak < 0.25) {
        return 1.0;
    }
    let mut g = 40f32.min(0.7 / peak);
    if noise_floor > 0.0 {
        g = g.min(4f32.max(0.02 / noise_floor));
    }
    1f32.max(g)
}

pub fn prepare(samples: &[f32], sample_rate: i32) -> Vec<f32> {
    if samples.is_empty() {
        return samples.to_vec();
    }
    let filtered = high_pass(samples, sample_rate, 80.0);
    let st = analyze(&filtered, sample_rate);
    let mut out = filtered;
    if let Some((lo, hi)) = st.speech_range {
        let lead = (0.3 * sample_rate as f64) as usize;
        let trail = (0.4 * sample_rate as f64) as usize;
        let a = lo.saturating_sub(lead);
        let b = out.len().min(hi + trail);
        if b - a >= (0.3 * sample_rate as f64) as usize {
            out = out[a..b].to_vec();
        }
    }
    let peak = out.iter().fold(0f32, |a, s| a.max(s.abs()));
    let g = gain(peak, st.noise_floor);
    if g > 1.0 {
        let frame = ((sample_rate as f64) * FRAME_SECONDS).max(1.0) as usize;
        let gate = (st.noise_floor * 3.0).max(peak * 0.06);
        let mut i = 0;
        while i < out.len() {
            let e = out.len().min(i + frame);
            let mut acc = 0f32;
            for s in &out[i..e] {
                acc += s * s;
            }
            let rms = (acc / (e - i) as f32).sqrt();
            let fg = if g >= 6.0 && rms < gate { g * 0.35 } else { g };
            for s in &mut out[i..e] {
                *s = (*s * fg).clamp(-1.0, 1.0);
            }
            i = e;
        }
    }
    out
}

const HALLUCINATIONS: &[&str] = &[
    "köszönöm a figyelmet",
    "köszönöm, hogy megnézted",
    "köszönöm szépen a figyelmet",
    "feliratok",
    "feliratozta",
    "a feliratot",
    "amara.org",
    "thanks for watching",
    "thank you for watching",
    "subtitles by",
    "subscribe",
    "iratkozz fel",
    "like-old",
    "www.",
    "transcribed by",
    "please subscribe",
];

pub fn clean_transcript(text: &str, _peak: f32, speech_seconds: Option<f64>) -> String {
    let mut t = text.trim().to_string();
    let lower = t.to_lowercase();
    let bare = lower.trim_matches(|c: char| c.is_whitespace() || crate::util::is_punct(c)).to_string();
    for h in HALLUCINATIONS {
        if bare.starts_with(h) || bare.ends_with(h) {
            if let Some(sp) = speech_seconds {
                if sp > 2.5 && bare != *h {
                    let l = t.to_lowercase();
                    if l.ends_with(h) || l.ends_with(&format!("{h}.")) || l.ends_with(&format!("{h}!")) {
                        if let Some(idx) = l.rfind(h) {
                            t = t[..idx].trim_matches(|c: char| c.is_whitespace() || c == ',').to_string();
                        }
                    }
                    continue;
                }
            }
            if bare.chars().count() <= h.chars().count() + 30 {
                return String::new();
            }
        }
    }
    if let Some(sp) = speech_seconds {
        if sp < 0.5 && ["köszönöm", "köszi", "thank you", "thanks", "you", "bye", "szia"].contains(&bare.as_str()) {
            return String::new();
        }
    }
    t
}

pub fn is_silent_raw(seconds: f64, peak: f32) -> bool {
    seconds < 0.35 || peak < 0.003
}

pub fn is_silent(st: &AudioStats) -> bool {
    if st.seconds >= 0.35 && is_whisper(st) {
        return false;
    }
    is_silent_raw(st.seconds, st.peak) || (st.speech_seconds < 0.15 && st.peak < 0.02)
}

pub fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_bytes = samples.len() * 2;
    let mut d = vec![0u8; 44 + data_bytes];
    d[0..4].copy_from_slice(b"RIFF");
    d[4..8].copy_from_slice(&(36 + data_bytes as u32).to_le_bytes());
    d[8..12].copy_from_slice(b"WAVE");
    d[12..16].copy_from_slice(b"fmt ");
    d[16..20].copy_from_slice(&16u32.to_le_bytes());
    d[20..22].copy_from_slice(&1u16.to_le_bytes());
    d[22..24].copy_from_slice(&1u16.to_le_bytes());
    d[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    d[28..32].copy_from_slice(&(sample_rate * 2).to_le_bytes());
    d[32..34].copy_from_slice(&2u16.to_le_bytes());
    d[34..36].copy_from_slice(&16u16.to_le_bytes());
    d[36..40].copy_from_slice(b"data");
    d[40..44].copy_from_slice(&(data_bytes as u32).to_le_bytes());
    for (i, s) in samples.iter().enumerate() {
        d[44 + i * 2..44 + i * 2 + 2].copy_from_slice(&s.to_le_bytes());
    }
    d
}

pub fn float_to_i16(samples: &[f32]) -> Vec<i16> {
    samples.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect()
}

pub fn resample_mono_16k(interleaved: &[f32], channels: u16, sample_rate: u32) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    if ch == 0 || interleaved.is_empty() {
        return vec![];
    }
    let frames = interleaved.len() / ch;
    let mut mono = Vec::with_capacity(frames);
    for i in 0..frames {
        let mut acc = 0f32;
        for c in 0..ch {
            acc += interleaved.get(i * ch + c).copied().unwrap_or(0.0);
        }
        mono.push(acc / ch as f32);
    }
    if sample_rate == 16_000 || sample_rate == 0 {
        return mono;
    }
    let out_len = ((mono.len() as f64) * 16_000.0 / sample_rate as f64).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * sample_rate as f64 / 16_000.0;
        let j = src.floor() as usize;
        let frac = (src - j as f64) as f32;
        let a = mono.get(j).copied().unwrap_or(0.0);
        let b = mono.get(j + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

pub fn env_value(contents: &str, key: &str) -> Option<String> {
    for raw in contents.lines() {
        let mut line = raw.trim().to_string();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim().to_string();
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        if k.trim() != key {
            continue;
        }
        let mut v = v.trim().to_string();
        if v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
            v = v[1..v.len() - 1].to_string();
        } else if let Some(i) = v.find(" #") {
            v = v[..i].trim().to_string();
        }
        return if v.is_empty() { None } else { Some(v) };
    }
    None
}

pub fn env_keys(contents: &str) -> Vec<String> {
    contents
        .lines()
        .filter_map(|raw| {
            let mut l = raw.trim().to_string();
            if l.starts_with('#') {
                return None;
            }
            if let Some(rest) = l.strip_prefix("export ") {
                l = rest.to_string();
            }
            let (k, _) = l.split_once('=')?;
            let k = k.trim();
            if k.is_empty() { None } else { Some(k.to_string()) }
        })
        .collect()
}
