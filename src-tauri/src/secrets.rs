use crate::identity::APP_ID;
use bnflow_core::{AIProvider, ProviderKeys};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub provider: String,
    pub label: String,
    pub has_key: bool,
    pub source: String,
    pub can_transcribe: bool,
    pub placeholder: String,
    pub key_page: String,
}

pub fn read_key(provider: AIProvider) -> Option<String> {
    if let Some(v) = keyring_get(provider) {
        return Some(v);
    }
    for name in provider.env_names() {
        if let Ok(v) = std::env::var(name) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub fn key_source(provider: AIProvider) -> &'static str {
    if keyring_get(provider).is_some() {
        "keychain"
    } else if provider.env_names().iter().any(|n| std::env::var(n).map(|v| !v.trim().is_empty()).unwrap_or(false)) {
        "env"
    } else {
        "none"
    }
}

pub fn set_key(provider: AIProvider, key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(APP_ID, provider.store_name()).map_err(store_err)?;
    let key = key.trim();
    if key.is_empty() {
        let _ = entry.delete_credential();
        return Ok(());
    }
    entry.set_password(key).map_err(store_err)
}

pub fn load_keys() -> ProviderKeys {
    let pairs = AIProvider::all().into_iter().filter_map(|p| read_key(p).map(|k| (p, k))).collect();
    ProviderKeys::from_owned(pairs)
}

pub fn describe_keys() -> Vec<KeyInfo> {
    AIProvider::all()
        .into_iter()
        .map(|p| KeyInfo {
            provider: match p {
                AIProvider::Openai => "openai",
                AIProvider::Anthropic => "anthropic",
                AIProvider::Gemini => "gemini",
            }
            .into(),
            label: p.label().into(),
            has_key: read_key(p).is_some(),
            source: key_source(p).into(),
            can_transcribe: p.can_transcribe(),
            placeholder: p.placeholder().into(),
            key_page: p.key_page().into(),
        })
        .collect()
}

pub fn parse_provider(name: &str) -> Option<AIProvider> {
    match name {
        "openai" => Some(AIProvider::Openai),
        "anthropic" => Some(AIProvider::Anthropic),
        "gemini" => Some(AIProvider::Gemini),
        _ => None,
    }
}

fn keyring_get(provider: AIProvider) -> Option<String> {
    let entry = keyring::Entry::new(APP_ID, provider.store_name()).ok()?;
    let v = entry.get_password().ok()?;
    let v = v.trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

fn store_err(err: impl std::fmt::Display) -> String {
    let _ = err;
    "A kulcsot nem sikerült a rendszer titkos tárolójába írni. Windows: Hitelesítőadat-kezelő, macOS: Kulcskarika.".into()
}
