use crate::models::{AppCategory, AppSettings, DictationContext};
use crate::prompts::Prompts;
use crate::text_pipeline::TextPipeline;
use crate::text_rules::{context_names, AppCategoryResolver};
use crate::util::{regex, word_count, words};
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowError {
    NoApiKey,
    Unauthorized,
    Quota,
    RateLimit,
    TooLarge,
    BadRequest(String),
    Server,
    Network,
    EmptyResult,
    Timeout,
}

impl FlowError {
    pub fn message(&self) -> String {
        match self {
            Self::NoApiKey => "Nincs beállítva API-kulcs (OpenAI vagy Gemini kell az átíráshoz).".into(),
            Self::Unauthorized => "Az API-kulcs érvénytelen.".into(),
            Self::Quota => "Elfogyott az átírás kerete. Nézd meg az OpenAI vagy a Gemini számlázását.".into(),
            Self::RateLimit => "Túl sok kérés, próbáld újra pár másodperc múlva.".into(),
            Self::TooLarge => "A felvétel túl hosszú.".into(),
            Self::BadRequest(_) => "A szolgáltatás elutasította a kérést.".into(),
            Self::Server => "Az átírási szolgáltatás most nem elérhető.".into(),
            Self::Network => "Nincs hálózati kapcsolat.".into(),
            Self::EmptyResult => "Nem érkezett szöveg.".into(),
            Self::Timeout => "Időtúllépés.".into(),
        }
    }
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::RateLimit | Self::Server | Self::Network | Self::Timeout)
    }
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete(&self, system: &str, user: &str, prediction: Option<&str>) -> Result<String, FlowError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    pub text: String,
    pub used_llm: bool,
    pub llm_rejected: bool,
}

pub struct DictationFormatter;

impl DictationFormatter {
    pub fn category(context: &DictationContext) -> AppCategory {
        AppCategoryResolver::category_for(context.bundle_id.as_deref(), context.window_title.as_deref())
    }

    pub async fn format(raw: &str, settings: &AppSettings, context: &DictationContext, chat: Option<&dyn ChatProvider>) -> FormatResult {
        let cat = Self::category(context);
        let tone = settings.tone_for(cat);
        let ctx = if settings.context_awareness {
            context.clone()
        } else {
            DictationContext {
                app_name: context.app_name.clone(),
                bundle_id: context.bundle_id.clone(),
                window_title: context.window_title.clone(),
                ..DictationContext::empty()
            }
        };
        let local = TextPipeline::process_with(raw, settings, tone, &ctx, cat);
        if !(settings.smart_formatting && chat.is_some() && FormatPolicy::needs_llm(raw)) {
            return FormatResult { text: local, used_llm: false, llm_rejected: false };
        }
        let chat = chat.unwrap();
        let mut terms: Vec<String> = settings.dictionary.iter().map(|d| d.term.clone()).collect();
        terms.extend(context_names(&ctx));
        let system = Prompts::smart_formatting(tone, &terms, cat);
        let user = Prompts::smart_formatting_user(raw, &ctx);
        let result = with_timeout(settings.llm_timeout_seconds, chat.complete(&system, &user, Some(&local))).await;
        let Some(Ok(out)) = result else {
            return FormatResult { text: local, used_llm: false, llm_rejected: false };
        };
        let cleaned = Prompts::clean_output(&out);
        if !LLMGuard::accept(raw, &local, &cleaned) {
            return FormatResult { text: local, used_llm: false, llm_rejected: true };
        }
        let mut post = settings.clone();
        post.remove_fillers = false;
        let final_text = TextPipeline::process_with(&cleaned, &post, tone, &ctx, cat);
        FormatResult { text: final_text, used_llm: true, llm_rejected: false }
    }
}

pub async fn with_timeout<T: Send>(seconds: f64, op: impl std::future::Future<Output = T> + Send) -> Option<T> {
    let dur = Duration::from_secs_f64(seconds.max(0.5));
    match tokio::time::timeout(dur, op).await {
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

pub struct FormatPolicy;

impl FormatPolicy {
    pub const SHORT_WORD_LIMIT: usize = 12;
    pub fn needs_llm(raw: &str) -> bool {
        let n = word_count(raw);
        if n <= 3 {
            return false;
        }
        if n > Self::SHORT_WORD_LIMIT {
            return true;
        }
        let padded = format!(" {} ", regex(r"[^\p{L}\p{N} ]").replace_all(&raw.to_lowercase(), " "));
        let padded = regex(r"\s+").replace_all(&padded, " ").to_string();
        const CUES: &[&str] = &[
            "vagyis", "mármint", "illetve", "helyesbítek", "bocsánat", "nem is", "várj", "inkább", "jobban mondva", "i mean", "actually", "scratch that", "sorry", "no wait", "rather", "először", "másodszor", "harmadszor", "egyrészt", "pontok", "lista", "egyes", "kettes", "first", "second", "third", "bullet", "számozott",
        ];
        if CUES.iter().any(|c| padded.contains(&format!(" {c} "))) {
            return true;
        }
        let toks: Vec<&str> = padded.split_whitespace().collect();
        if toks.windows(2).any(|w| w[0] == w[1]) {
            return true;
        }
        const FILL: &[&str] = &["öhm", "ömm", "hmm", "izé", "um", "uh", "ö", "öö"];
        if toks.iter().any(|t| FILL.contains(t)) {
            return true;
        }
        let has_punct = regex(r"[.?!,]").is_match(raw).unwrap_or(false);
        !has_punct && n > 8
    }
}

pub struct LLMGuard;

impl LLMGuard {
    fn content_stems(s: &str) -> Vec<String> {
        words(s).into_iter().filter(|w| w.chars().count() >= 4).map(|w| w.chars().take(4).collect()).collect()
    }

    pub fn accept(raw: &str, local: &str, output: &str) -> bool {
        let out = output.trim();
        if out.is_empty() {
            return false;
        }
        let rw = word_count(local).max(1);
        let ow = word_count(out);
        let lo = rw as f64 * 0.3 - 2.0;
        let hi = rw as f64 * 1.8 + 3.0;
        if (ow as f64) < lo || (ow as f64) > hi {
            return false;
        }
        let src: std::collections::HashSet<String> = Self::content_stems(&format!("{raw} {local}")).into_iter().collect();
        let dst = Self::content_stems(out);
        if !dst.is_empty() {
            let novel = dst.iter().filter(|s| !src.contains(*s)).count();
            if novel as f64 / dst.len() as f64 > 0.4 {
                return false;
            }
        }
        let src_stems: std::collections::HashSet<String> = Self::content_stems(local).into_iter().collect();
        if !src_stems.is_empty() {
            let dst_set: std::collections::HashSet<&String> = dst.iter().collect();
            let kept = src_stems.iter().filter(|s| dst_set.contains(s)).count();
            if (kept as f64) / (src_stems.len() as f64) < 0.6 {
                return false;
            }
        }
        if local.contains('?') && !out.contains('?') {
            return false;
        }
        true
    }
}
