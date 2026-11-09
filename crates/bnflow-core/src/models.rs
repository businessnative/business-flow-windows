use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DictationLanguage {
    Auto,
    Hu,
    En,
}

impl DictationLanguage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatikus (magyar + angol)",
            Self::Hu => "Magyar",
            Self::En => "Angol",
        }
    }
    pub fn codes(self) -> &'static [&'static str] {
        match self {
            Self::Auto => &["hu", "en"],
            Self::Hu => &["hu"],
            Self::En => &["en"],
        }
    }
}

impl Default for DictationLanguage {
    fn default() -> Self {
        // The cross-platform app defaults to Hungarian (the Mac app defaults to auto).
        Self::Hu
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleTone {
    Formal,
    Casual,
    VeryCasual,
}

impl Default for StyleTone {
    fn default() -> Self {
        Self::Formal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppCategory {
    Personal,
    Work,
    Email,
    Other,
}

impl Default for AppCategory {
    fn default() -> Self {
        Self::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModifierKey {
    Fn,
    LeftControl,
    RightControl,
    LeftOption,
    RightOption,
    LeftCommand,
    RightCommand,
    LeftShift,
    RightShift,
}

impl ModifierKey {
    pub fn modifier_class(self) -> ModifierClass {
        match self {
            Self::Fn => ModifierClass::Fn,
            Self::LeftControl | Self::RightControl => ModifierClass::Control,
            Self::LeftOption | Self::RightOption => ModifierClass::Option,
            Self::LeftCommand | Self::RightCommand => ModifierClass::Command,
            Self::LeftShift | Self::RightShift => ModifierClass::Shift,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::LeftControl => "⌃",
            Self::RightControl => "jobb ⌃",
            Self::LeftOption => "⌥",
            Self::RightOption => "jobb ⌥",
            Self::LeftCommand => "⌘",
            Self::RightCommand => "jobb ⌘",
            Self::LeftShift => "⇧",
            Self::RightShift => "jobb ⇧",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModifierClass {
    Fn,
    Control,
    Option,
    Shift,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ShortcutTrigger {
    Hold { keys: Vec<ModifierKey> },
    Combo { key_code: u16, modifiers: Vec<ModifierClass> },
    Mouse { button: i32 },
}

impl ShortcutTrigger {
    pub fn hold(keys: &[ModifierKey]) -> Self {
        Self::Hold { keys: keys.to_vec() }
    }
    pub fn combo(key_code: u16, modifiers: &[ModifierClass]) -> Self {
        Self::Combo { key_code, modifiers: modifiers.to_vec() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutSettings {
    pub push_to_talk: Vec<ShortcutTrigger>,
    pub hands_free: Vec<ShortcutTrigger>,
    pub command: Vec<ShortcutTrigger>,
    pub paste_last: Vec<ShortcutTrigger>,
    pub polish: Vec<ShortcutTrigger>,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self::mac_default()
    }
}

impl ShortcutSettings {
    /// Same defaults as the Mac app (fn hold, fn+Space toggle).
    pub fn mac_default() -> Self {
        Self {
            push_to_talk: vec![ShortcutTrigger::hold(&[ModifierKey::Fn])],
            hands_free: vec![ShortcutTrigger::combo(49, &[ModifierClass::Fn])],
            command: vec![ShortcutTrigger::hold(&[ModifierKey::Fn, ModifierKey::LeftControl])],
            paste_last: vec![ShortcutTrigger::combo(9, &[ModifierClass::Control, ModifierClass::Command])],
            polish: vec![ShortcutTrigger::combo(35, &[ModifierClass::Control, ModifierClass::Command])],
        }
    }

    /// Windows has no reliable fn key. Right Ctrl hold, Ctrl+Space toggle.
    pub fn windows_default() -> Self {
        Self {
            push_to_talk: vec![ShortcutTrigger::hold(&[ModifierKey::RightControl])],
            hands_free: vec![ShortcutTrigger::combo(49, &[ModifierClass::Control])],
            command: vec![],
            paste_last: vec![ShortcutTrigger::combo(9, &[ModifierClass::Control, ModifierClass::Shift])],
            polish: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DictionaryEntry {
    #[serde(default = "new_id")]
    pub id: Uuid,
    pub term: String,
    #[serde(default)]
    pub heard_as: Vec<String>,
    #[serde(default)]
    pub auto_learned: bool,
}

fn new_id() -> Uuid {
    Uuid::new_v4()
}

impl DictionaryEntry {
    pub fn new(term: impl Into<String>, heard_as: &[&str]) -> Self {
        Self {
            id: Uuid::new_v4(),
            term: term.into(),
            heard_as: heard_as.iter().map(|s| (*s).to_string()).collect(),
            auto_learned: false,
        }
    }
}

impl Default for DictionaryEntry {
    fn default() -> Self {
        Self::new("", &[])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Snippet {
    #[serde(default = "new_id")]
    pub id: Uuid,
    pub trigger: String,
    pub expansion: String,
}

impl Default for Snippet {
    fn default() -> Self {
        Self { id: Uuid::new_v4(), trigger: String::new(), expansion: String::new() }
    }
}

impl Snippet {
    pub fn new(trigger: impl Into<String>, expansion: impl Into<String>) -> Self {
        Self { id: Uuid::new_v4(), trigger: trigger.into(), expansion: expansion.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryKind {
    Dictation,
    Command,
    Polish,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: Uuid,
    pub date_ms: i64,
    pub text: String,
    pub raw_text: String,
    pub app_name: String,
    pub duration_seconds: f64,
}

impl HistoryItem {
    pub fn new(text: String, raw_text: String, app_name: String, duration_seconds: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            date_ms: chrono_now_ms(),
            text,
            raw_text,
            app_name,
            duration_seconds,
        }
    }
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AIProvider {
    Openai,
    Anthropic,
    Gemini,
}

impl AIProvider {
    pub fn all() -> [AIProvider; 3] {
        [Self::Openai, Self::Anthropic, Self::Gemini]
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Openai => "OpenAI",
            Self::Anthropic => "Anthropic Claude",
            Self::Gemini => "Google Gemini",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Self::Openai => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
        }
    }
    pub fn store_name(self) -> &'static str {
        match self {
            Self::Openai => "openai-api-key",
            Self::Anthropic => "anthropic-api-key",
            Self::Gemini => "gemini-api-key",
        }
    }
    pub fn env_names(self) -> &'static [&'static str] {
        match self {
            Self::Openai => &["OPENAI_API_KEY"],
            Self::Anthropic => &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY"],
            Self::Gemini => &["GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_GENERATIVE_AI_API_KEY"],
        }
    }
    pub fn for_env_name(name: &str) -> Option<Self> {
        let u = name.to_uppercase();
        if u.contains("OPENAI") {
            Some(Self::Openai)
        } else if u.contains("ANTHROPIC") || u.contains("CLAUDE") {
            Some(Self::Anthropic)
        } else if u.contains("GEMINI") || u.contains("GOOGLE") {
            Some(Self::Gemini)
        } else {
            None
        }
    }
    pub fn key_page(self) -> &'static str {
        match self {
            Self::Openai => "https://platform.openai.com/api-keys",
            Self::Anthropic => "https://console.anthropic.com/settings/keys",
            Self::Gemini => "https://aistudio.google.com/apikey",
        }
    }
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Openai => "sk-…",
            Self::Anthropic => "sk-ant-…",
            Self::Gemini => "AIza…",
        }
    }
    /// Claude has no speech-to-text API.
    pub fn can_transcribe(self) -> bool {
        self != Self::Anthropic
    }
    pub fn transcription_models(self) -> &'static [&'static str] {
        match self {
            Self::Openai => &["gpt-transcribe", "gpt-4o-mini-transcribe", "gpt-4o-transcribe"],
            Self::Gemini => &["gemini-3.5-flash-lite", "gemini-3.8-flash", "gemini-2.5-flash"],
            Self::Anthropic => &[],
        }
    }
    pub fn chat_models(self) -> &'static [&'static str] {
        match self {
            Self::Openai => &["gpt-5.6-luna", "gpt-4.1-mini", "gpt-4.1-nano"],
            Self::Anthropic => &["claude-haiku-5-5", "claude-sonnet-5-5"],
            Self::Gemini => &["gemini-3.5-flash-lite", "gemini-3.8-flash", "gemini-2.5-flash-lite"],
        }
    }
    pub fn transcribers() -> Vec<AIProvider> {
        Self::all().into_iter().filter(|p| p.can_transcribe()).collect()
    }
}

impl Default for AIProvider {
    fn default() -> Self {
        Self::Openai
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderKeys {
    values: HashMap<AIProvider, String>,
}

impl ProviderKeys {
    pub fn new(pairs: &[(AIProvider, &str)]) -> Self {
        Self::from_owned(pairs.iter().map(|(p, k)| (*p, (*k).to_string())).collect())
    }

    pub fn from_owned(pairs: Vec<(AIProvider, String)>) -> Self {
        let mut values = HashMap::new();
        for (p, k) in pairs {
            if !k.is_empty() {
                values.insert(p, k);
            }
        }
        Self { values }
    }
    pub fn get(&self, p: AIProvider) -> Option<&str> {
        self.values.get(&p).map(|s| s.as_str())
    }
    pub fn can_transcribe(&self) -> bool {
        AIProvider::transcribers().into_iter().any(|p| self.values.contains_key(&p))
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub language: DictationLanguage,
    pub shortcuts: ShortcutSettings,
    pub styles: HashMap<AppCategory, StyleTone>,
    pub dictionary: Vec<DictionaryEntry>,
    pub snippets: Vec<Snippet>,
    pub smart_formatting: bool,
    pub remove_fillers: bool,
    pub voice_commands: bool,
    pub launch_at_login: bool,
    pub microphone_name: Option<String>,
    pub history_enabled: bool,
    pub transcription_model: String,
    pub fallback_transcription_model: String,
    pub chat_model: String,
    pub fallback_chat_model: String,
    pub transcription_provider: AIProvider,
    pub cleanup_provider: AIProvider,
    pub gemini_transcription_model: String,
    pub gemini_chat_model: String,
    pub anthropic_chat_model: String,
    pub context_awareness: bool,
    pub llm_timeout_seconds: f64,
    pub onboarding_done: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut styles = HashMap::new();
        styles.insert(AppCategory::Personal, StyleTone::Casual);
        styles.insert(AppCategory::Work, StyleTone::Casual);
        styles.insert(AppCategory::Email, StyleTone::Formal);
        styles.insert(AppCategory::Other, StyleTone::Formal);
        Self {
            language: DictationLanguage::Hu,
            shortcuts: ShortcutSettings::default(),
            styles,
            dictionary: vec![
                DictionaryEntry::new("Business Native", &[]),
                DictionaryEntry::new("businessnative.hu", &[]),
            ],
            snippets: vec![],
            smart_formatting: true,
            remove_fillers: true,
            voice_commands: true,
            launch_at_login: false,
            microphone_name: None,
            history_enabled: true,
            transcription_model: "gpt-transcribe".into(),
            fallback_transcription_model: "gpt-4o-transcribe".into(),
            chat_model: "gpt-5.6-luna".into(),
            fallback_chat_model: "gpt-4.1-mini".into(),
            transcription_provider: AIProvider::Openai,
            cleanup_provider: AIProvider::Openai,
            gemini_transcription_model: "gemini-3.5-flash-lite".into(),
            gemini_chat_model: "gemini-3.5-flash-lite".into(),
            anthropic_chat_model: "claude-haiku-5-5".into(),
            context_awareness: true,
            llm_timeout_seconds: 8.0,
            onboarding_done: false,
        }
    }
}

impl AppSettings {
    pub fn tone_for(&self, category: AppCategory) -> StyleTone {
        self.styles.get(&category).copied().unwrap_or(StyleTone::Formal)
    }

    /// First-run settings: Hungarian, login launch off, platform hotkey.
    pub fn fresh_for_platform(windows: bool) -> Self {
        let mut s = Self::default();
        s.shortcuts = if windows { ShortcutSettings::windows_default() } else { ShortcutSettings::mac_default() };
        s.launch_at_login = false;
        s.language = DictationLanguage::Hu;
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DictationContext {
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub before: String,
    pub after: String,
    pub selected: Option<String>,
    pub is_secure: bool,
}

impl DictationContext {
    pub fn empty() -> Self {
        Self::default()
    }
}

/// The Mac app's eval dictionary, used by the ported text-rule fixtures.
pub fn eval_dictionary() -> Vec<DictionaryEntry> {
    vec![
        DictionaryEntry::new("BusinessNative", &["biznisz nétív", "business native", "biznisznétív"]),
        DictionaryEntry::new("ClickUp", &[]),
        DictionaryEntry::new("Supabase", &[]),
        DictionaryEntry::new("Vercel", &["versel", "verszel"]),
        DictionaryEntry::new("Dorottya", &["Dorotya"]),
    ]
}
