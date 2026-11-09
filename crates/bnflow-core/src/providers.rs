use crate::formatter::FlowError;
use crate::models::{AIProvider, AppSettings, DictationLanguage, ProviderKeys};
use crate::prompts::Prompts;
use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BuiltRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl BuiltRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn send(&self, req: BuiltRequest) -> Result<Vec<u8>, FlowError>;
}

pub struct Multipart {
    boundary: String,
    body: Vec<u8>,
}

impl Multipart {
    pub fn new() -> Self {
        Self { boundary: format!("bnflow-{}", uuid::Uuid::new_v4().simple()), body: Vec::new() }
    }
    pub fn field(&mut self, name: &str, value: &str) {
        self.body.extend(format!("--{}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n", self.boundary).as_bytes());
    }
    pub fn file(&mut self, name: &str, filename: &str, mime: &str, data: &[u8]) {
        self.body.extend(format!("--{}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n", self.boundary).as_bytes());
        self.body.extend(data);
        self.body.extend(b"\r\n");
    }
    pub fn finish(mut self) -> (String, Vec<u8>) {
        self.body.extend(format!("--{}--\r\n", self.boundary).as_bytes());
        (format!("multipart/form-data; boundary={}", self.boundary), self.body)
    }
}

impl Default for Multipart {
    fn default() -> Self {
        Self::new()
    }
}

pub fn uses_multi_language_fields(model: &str) -> bool {
    model.starts_with("gpt-transcribe")
}

pub fn is_reasoning_model(model: &str) -> bool {
    model.starts_with("gpt-5") || model.starts_with("o1") || model.starts_with("o3") || model.starts_with("o4")
}

pub fn reasoning_effort() -> Option<String> {
    match std::env::var("BF_REASONING_EFFORT") {
        Ok(v) if v == "off" => None,
        Ok(v) => Some(v),
        Err(_) => Some("low".into()),
    }
}

pub fn make_transcription_request(api_key: &str, wav: &[u8], model: &str, language: DictationLanguage, terms: &[String]) -> BuiltRequest {
    let mut m = Multipart::new();
    m.file("file", "dictation.wav", "audio/wav", wav);
    m.field("model", model);
    m.field("prompt", &Prompts::transcription(language, terms));
    if uses_multi_language_fields(model) {
        for l in language.codes() {
            m.field("languages[]", l);
        }
        for t in terms.iter().filter(|t| !t.is_empty()).take(32) {
            m.field("keywords[]", t);
        }
    } else if language != DictationLanguage::Auto {
        let code = match language {
            DictationLanguage::Hu => "hu",
            DictationLanguage::En => "en",
            DictationLanguage::Auto => "hu",
        };
        m.field("language", code);
    }
    m.field("response_format", "json");
    let (ct, body) = m.finish();
    BuiltRequest {
        method: "POST".into(),
        url: "https://api.openai.com/v1/audio/transcriptions".into(),
        headers: vec![("Authorization".into(), format!("Bearer {api_key}")), ("Content-Type".into(), ct)],
        body,
    }
}

pub fn make_chat_request(api_key: &str, model: &str, system: &str, user: &str, effort: bool, prediction: Option<&str>) -> BuiltRequest {
    let mut body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    if effort {
        if is_reasoning_model(model) {
            if let Some(e) = reasoning_effort() {
                body["reasoning_effort"] = json!(e);
            }
        }
    }
    if let Some(prediction) = prediction {
        if !prediction.is_empty() {
            body["prediction"] = json!({"type": "content", "content": prediction});
        }
    }
    BuiltRequest {
        method: "POST".into(),
        url: "https://api.openai.com/v1/chat/completions".into(),
        headers: vec![("Authorization".into(), format!("Bearer {api_key}")), ("Content-Type".into(), "application/json".into())],
        body: serde_json::to_vec(&body).unwrap_or_default(),
    }
}

pub fn make_anthropic_request(api_key: &str, model: &str, system: &str, user: &str) -> BuiltRequest {
    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": system,
        "messages": [{"role": "user", "content": user}],
        "output_config": {"effort": "low"}
    });
    BuiltRequest {
        method: "POST".into(),
        url: "https://api.anthropic.com/v1/messages".into(),
        headers: vec![
            ("x-api-key".into(), api_key.into()),
            ("anthropic-version".into(), "2023-06-01".into()),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: serde_json::to_vec(&body).unwrap_or_default(),
    }
}

pub fn gemini_transcription_instruction(language: DictationLanguage, terms: &[String]) -> String {
    let mut s = "Írd le szó szerint a hangfelvételen elhangzó beszédet. Csak az elhangzott szöveget add vissza, magyarázat, idézőjel és időbélyeg nélkül. Ha nincs beszéd, adj vissza üres választ.".to_string();
    if language == DictationLanguage::Auto {
        s.push_str(" A beszéd magyar vagy angol (vegyesen is lehet); az angol szavakat angolul írd.");
    } else {
        s.push_str(&format!(" A beszéd nyelve: {}.", language.codes().join(",")));
    }
    let t: Vec<&str> = terms.iter().map(|s| s.as_str()).take(80).collect();
    if !t.is_empty() {
        s.push_str(&format!(" Helyes írásmód ezekre a kifejezésekre: {}.", t.join(", ")));
    }
    s
}

pub fn make_gemini_chat_request(api_key: &str, model: &str, system: &str, user: &str) -> BuiltRequest {
    let body = json!({
        "systemInstruction": {"parts": [{"text": system}]},
        "contents": [{"role": "user", "parts": [{"text": user}]}],
        "generationConfig": {"temperature": 0.2}
    });
    gemini_post(model, api_key, body)
}

pub fn make_gemini_transcription_request(api_key: &str, model: &str, wav: &[u8], language: DictationLanguage, terms: &[String]) -> BuiltRequest {
    let b64 = base64::engine::general_purpose::STANDARD.encode(wav);
    let body = json!({
        "contents": [{"role": "user", "parts": [
            {"inlineData": {"mimeType": "audio/wav", "data": b64}},
            {"text": gemini_transcription_instruction(language, terms)}
        ]}],
        "generationConfig": {"temperature": 0}
    });
    gemini_post(model, api_key, body)
}

fn gemini_post(model: &str, api_key: &str, body: Value) -> BuiltRequest {
    BuiltRequest {
        method: "POST".into(),
        url: format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"),
        headers: vec![("x-goog-api-key".into(), api_key.into()), ("Content-Type".into(), "application/json".into())],
        body: serde_json::to_vec(&body).unwrap_or_default(),
    }
}

pub fn make_key_test_request(p: AIProvider, key: &str) -> BuiltRequest {
    match p {
        AIProvider::Openai => BuiltRequest {
            method: "GET".into(),
            url: "https://api.openai.com/v1/models".into(),
            headers: vec![("Authorization".into(), format!("Bearer {key}"))],
            body: vec![],
        },
        AIProvider::Anthropic => BuiltRequest {
            method: "GET".into(),
            url: "https://api.anthropic.com/v1/models".into(),
            headers: vec![
                ("x-api-key".into(), key.into()),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("Content-Type".into(), "application/json".into()),
            ],
            body: vec![],
        },
        AIProvider::Gemini => BuiltRequest {
            method: "GET".into(),
            url: "https://generativelanguage.googleapis.com/v1beta/models".into(),
            headers: vec![("x-goog-api-key".into(), key.into())],
            body: vec![],
        },
    }
}

pub fn map_error(status: u16, body: &[u8]) -> FlowError {
    let json: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let err = &json["error"];
    let code = err["code"].as_str().or_else(|| err["type"].as_str()).unwrap_or("");
    let param = err["param"].as_str().unwrap_or("");
    if status == 400 || status == 404 || status == 422 {
        let token = if param.is_empty() { code.to_string() } else { format!("{code} {param}") };
        return FlowError::BadRequest(token);
    }
    match status {
        401 | 403 => FlowError::Unauthorized,
        402 => FlowError::Quota,
        429 => {
            if code.contains("insufficient_quota") || code.contains("credit_balance") {
                FlowError::Quota
            } else {
                FlowError::RateLimit
            }
        }
        413 => FlowError::TooLarge,
        _ => FlowError::Server,
    }
}

pub fn parse_transcription(data: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(data).ok()?;
    v["text"].as_str().map(|s| s.to_string())
}

pub fn parse_chat(data: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(data).ok()?;
    let msg = &v["choices"][0]["message"]["content"];
    if let Some(s) = msg.as_str() {
        return Some(s.to_string());
    }
    if let Some(parts) = msg.as_array() {
        let s: String = parts.iter().filter_map(|p| p["text"].as_str()).collect();
        return Some(s);
    }
    None
}

pub fn parse_anthropic(data: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(data).ok()?;
    let parts = v["content"].as_array()?;
    let t: String = parts.iter().filter(|p| p["type"].as_str() == Some("text")).filter_map(|p| p["text"].as_str()).collect();
    let s = t.trim();
    if s.is_empty() { None } else { Some(s.to_string()) }
}

pub fn parse_gemini(data: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(data).ok()?;
    let parts = v["candidates"][0]["content"]["parts"].as_array()?;
    let s: String = parts.iter().filter(|p| p["thought"].as_bool() != Some(true)).filter_map(|p| p["text"].as_str()).collect();
    let s = s.trim();
    if s.is_empty() { None } else { Some(s.to_string()) }
}

fn rejected_predictions() -> &'static Mutex<Vec<String>> {
    static R: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Vec::new()))
}

fn prediction_allowed(model: &str) -> bool {
    !rejected_predictions().lock().map(|g| g.iter().any(|m| m == model)).unwrap_or(false)
}

fn reject_prediction(model: &str) {
    if let Ok(mut g) = rejected_predictions().lock() {
        if !g.iter().any(|m| m == model) {
            g.push(model.to_string());
        }
    }
}

pub fn chat_models(p: AIProvider, s: &AppSettings) -> (String, String) {
    match p {
        AIProvider::Openai => (s.chat_model.clone(), s.fallback_chat_model.clone()),
        AIProvider::Anthropic => {
            let fb = p.chat_models().iter().copied().find(|m| *m != s.anthropic_chat_model).unwrap_or(&s.anthropic_chat_model);
            (s.anthropic_chat_model.clone(), fb.to_string())
        }
        AIProvider::Gemini => {
            let fb = p.chat_models().iter().copied().find(|m| *m != s.gemini_chat_model).unwrap_or(&s.gemini_chat_model);
            (s.gemini_chat_model.clone(), fb.to_string())
        }
    }
}

pub fn chat_order(s: &AppSettings, keys: &ProviderKeys) -> Vec<AIProvider> {
    let mut order = vec![s.cleanup_provider];
    order.extend(AIProvider::all().into_iter().filter(|p| *p != s.cleanup_provider));
    order.into_iter().filter(|p| keys.get(*p).is_some()).collect()
}

pub fn transcription_order(s: &AppSettings, keys: &ProviderKeys) -> Vec<AIProvider> {
    let first = if s.transcription_provider.can_transcribe() { s.transcription_provider } else { AIProvider::Openai };
    let mut order = vec![first];
    order.extend(AIProvider::transcribers().into_iter().filter(|p| *p != first));
    order.into_iter().filter(|p| keys.get(*p).is_some()).collect()
}

#[derive(Clone)]
pub struct AIRouter<H: HttpClient> {
    pub http: H,
    pub retry_delay: Duration,
}

impl<H: HttpClient> AIRouter<H> {
    pub fn new(http: H) -> Self {
        Self { http, retry_delay: Duration::from_millis(600) }
    }

    pub async fn test_key(&self, p: AIProvider, key: &str) -> Result<(), FlowError> {
        self.http.send(make_key_test_request(p, key)).await.map(|_| ())
    }

    pub async fn transcribe(&self, s: &AppSettings, keys: &ProviderKeys, wav: &[u8], terms: &[String]) -> (Result<String, FlowError>, Option<AIProvider>) {
        let order = transcription_order(s, keys);
        if order.is_empty() {
            return (Err(FlowError::NoApiKey), None);
        }
        let mut last = (Err(FlowError::NoApiKey), None);
        for p in order {
            let key = keys.get(p).unwrap_or("");
            let r = if p == AIProvider::Openai {
                self.openai_transcribe(key, wav, &s.transcription_model, &s.fallback_transcription_model, s.language, terms).await
            } else {
                self.gemini_transcribe(key, &s.gemini_transcription_model, wav, s.language, terms).await
            };
            match r {
                Ok(text) => return (Ok(text), Some(p)),
                Err(FlowError::EmptyResult) => return (Err(FlowError::EmptyResult), Some(p)),
                Err(e) => last = (Err(e), Some(p)),
            }
        }
        last
    }

    async fn openai_transcribe(&self, key: &str, wav: &[u8], model: &str, fallback: &str, language: DictationLanguage, terms: &[String]) -> Result<String, FlowError> {
        let mut models = vec![model.to_string()];
        if !fallback.is_empty() && fallback != model {
            models.push(fallback.to_string());
        }
        let mut last = FlowError::Server;
        for (i, m) in models.iter().enumerate() {
            let mut attempt = 0;
            loop {
                match self.http.send(make_transcription_request(key, wav, m, language, terms)).await {
                    Ok(d) => match parse_transcription(&d) {
                        Some(t) => {
                            let clean = t.trim();
                            return if clean.is_empty() { Err(FlowError::EmptyResult) } else { Ok(clean.to_string()) };
                        }
                        None => {
                            last = FlowError::Server;
                            break;
                        }
                    },
                    Err(e) => {
                        let retryable = e.is_retryable();
                        last = e;
                        if retryable && attempt == 0 {
                            attempt += 1;
                            if !self.retry_delay.is_zero() {
                                tokio::time::sleep(self.retry_delay).await;
                            }
                            continue;
                        }
                        break;
                    }
                }
            }
            if matches!(last, FlowError::BadRequest(_)) && i + 1 < models.len() {
                continue;
            }
            return Err(clone_err(&last));
        }
        Err(last)
    }

    async fn gemini_transcribe(&self, key: &str, model: &str, wav: &[u8], language: DictationLanguage, terms: &[String]) -> Result<String, FlowError> {
        let models: Vec<String> = if model == "gemini-3.5-flash-lite" {
            vec![model.to_string()]
        } else {
            vec![model.to_string(), "gemini-3.5-flash-lite".into()]
        };
        let mut last = FlowError::EmptyResult;
        for m in models {
            match self.http.send(make_gemini_transcription_request(key, &m, wav, language, terms)).await {
                Ok(d) => return parse_gemini(&d).map(Ok).unwrap_or(Err(FlowError::EmptyResult)),
                Err(e) => {
                    let bad = matches!(e, FlowError::BadRequest(_));
                    last = e;
                    if bad {
                        continue;
                    }
                    return Err(clone_err(&last));
                }
            }
        }
        Err(last)
    }

    pub async fn chat(&self, p: AIProvider, key: &str, model: &str, fallback_model: &str, system: &str, user: &str, prediction: Option<&str>) -> Result<String, FlowError> {
        if p == AIProvider::Openai {
            return self.openai_chat(key, model, fallback_model, system, user, prediction).await;
        }
        let models: Vec<String> = if model == fallback_model {
            vec![model.to_string()]
        } else {
            vec![model.to_string(), fallback_model.to_string()]
        };
        let mut last = FlowError::EmptyResult;
        for m in models {
            let req = if p == AIProvider::Anthropic {
                make_anthropic_request(key, &m, system, user)
            } else {
                make_gemini_chat_request(key, &m, system, user)
            };
            match self.http.send(req).await {
                Ok(d) => {
                    let parsed = if p == AIProvider::Anthropic { parse_anthropic(&d) } else { parse_gemini(&d) };
                    return parsed.map(Ok).unwrap_or(Err(FlowError::EmptyResult));
                }
                Err(e) => {
                    let bad = matches!(e, FlowError::BadRequest(_));
                    last = e;
                    if bad {
                        continue;
                    }
                    return Err(clone_err(&last));
                }
            }
        }
        Err(last)
    }

    async fn openai_chat(&self, key: &str, model: &str, fallback: &str, system: &str, user: &str, prediction: Option<&str>) -> Result<String, FlowError> {
        let mut models = vec![model.to_string()];
        if !fallback.is_empty() && fallback != model {
            models.push(fallback.to_string());
        }
        let mut last = FlowError::Server;
        for m in models {
            let want = prediction.map(|p| !p.is_empty()).unwrap_or(false) && prediction_allowed(&m);
            let pred = if want { prediction } else { None };
            let mut res = self.http.send(make_chat_request(key, &m, system, user, true, pred)).await;
            if let Err(FlowError::BadRequest(ref c)) = res {
                if pred.is_some() && !c.contains("reasoning") {
                    let code = c.clone();
                    res = self.http.send(make_chat_request(key, &m, system, user, true, None)).await;
                    if res.is_ok() && (code.contains("prediction") || code.contains("unknown") || code.is_empty()) {
                        reject_prediction(&m);
                    }
                }
            }
            if let Err(FlowError::BadRequest(ref c)) = res {
                if is_reasoning_model(&m) && c.contains("reasoning") {
                    res = self.http.send(make_chat_request(key, &m, system, user, false, None)).await;
                }
            }
            match res {
                Ok(d) => match parse_chat(&d) {
                    Some(t) => {
                        let clean = crate::prompts::Prompts::clean_output(&t);
                        return if clean.is_empty() { Err(FlowError::EmptyResult) } else { Ok(clean) };
                    }
                    None => {
                        last = FlowError::Server;
                        continue;
                    }
                },
                Err(e) => {
                    let bad = matches!(e, FlowError::BadRequest(_));
                    let retryable = e.is_retryable();
                    last = e;
                    if bad || retryable {
                        continue;
                    }
                    return Err(clone_err(&last));
                }
            }
        }
        Err(last)
    }

    pub async fn complete(&self, s: &AppSettings, keys: &ProviderKeys, system: &str, user: &str, prediction: Option<&str>) -> Result<String, FlowError> {
        let order = chat_order(s, keys);
        if order.is_empty() {
            return Err(FlowError::NoApiKey);
        }
        let mut last = FlowError::NoApiKey;
        for p in order {
            let key = keys.get(p).unwrap_or("");
            let (model, fb) = chat_models(p, s);
            let pred = if p == AIProvider::Openai { prediction } else { None };
            match self.chat(p, key, &model, &fb, system, user, pred).await {
                Ok(text) => return Ok(text),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

fn clone_err(e: &FlowError) -> FlowError {
    e.clone()
}
