use bnflow_core::audio::{self, analyze, clean_transcript, encode_wav};
use bnflow_core::hotkey::{self, HotkeyAction, HotkeyEngine, HotkeyState};
use bnflow_core::providers::*;
use bnflow_core::*;
use std::collections::HashSet;
use std::sync::Mutex;

fn settings() -> AppSettings {
    let mut s = AppSettings::default();
    s.language = DictationLanguage::Auto;
    s.dictionary = eval_dictionary();
    s
}

fn mods(keys: &[ModifierKey]) -> HashSet<ModifierKey> {
    keys.iter().copied().collect()
}

#[test]
fn test_language() {
    assert_eq!(TextRules::language("Holnap átküldöm az ajánlatot"), DictationLanguage::Hu);
    assert_eq!(TextRules::language("I will send the offer tomorrow"), DictationLanguage::En);
}

#[test]
fn test_backtrack_numbers_and_names() {
    assert_eq!(TextRules::apply_backtrack("Holnap tíz órakor, nem, inkább tizenegykor találkozzunk."), "Holnap tizenegykor találkozzunk.");
    assert_eq!(TextRules::apply_backtrack("Let's meet on Tuesday, actually no, Wednesday at 3 pm."), "Let's meet on Wednesday at 3 pm.");
    assert_eq!(
        TextRules::apply_backtrack("Szia Anna, mikor érsz rá? Ezt töröld. Szia Anna, ráérsz holnap?"),
        "Szia Anna, ráérsz holnap?"
    );
    assert_eq!(TextRules::apply_backtrack("Nem inkább holnap?"), "Nem inkább holnap?");
}

#[test]
fn test_lists() {
    let hu = TextRules::apply_lists("A teendők első a számla második az ajánlat és harmadik a szerződés");
    assert_eq!(hu, "A teendők:\n1. Számla\n2. Ajánlat\n3. Szerződés");
    assert_eq!(
        TextRules::apply_lists("The agenda is first budget, second hiring and third the roadmap"),
        "The agenda is:\n1. Budget\n2. Hiring\n3. Roadmap"
    );
    assert_eq!(TextRules::apply_lists("Az első alkalommal jó volt"), "Az első alkalommal jó volt");
}

#[test]
fn test_hungarian_commas_and_question() {
    assert_eq!(TextRules::apply_hungarian_commas("Azt hiszem hogy holnap jövök"), "Azt hiszem, hogy holnap jövök");
    assert_eq!(TextRules::apply_hungarian_commas("Jövök és hogy mikor"), "Jövök és hogy mikor");
    assert_eq!(
        TextRules::apply_hungarian_commas("Az ügyfél aki tegnap írt még nem fizetett"),
        "Az ügyfél, aki tegnap írt, még nem fizetett"
    );
    assert_eq!(TextRules::apply_question_mark("Mikor tudnál ránézni a kampányra"), "Mikor tudnál ránézni a kampányra?");
    assert_eq!(TextRules::apply_question_mark("Holnap jövök"), "Holnap jövök");
    assert_eq!(TextRules::apply_question_mark("Megvan-e a számla"), "Megvan-e a számla?");
    assert_eq!(TextRules::apply_question_mark("Can you send it today"), "Can you send it today?");
}

#[test]
fn test_quotes_accents_addresses() {
    assert_eq!(TextRules::apply_hungarian_quotes("Azt írta: \"Kész vagyunk.\""), "Azt írta: „Kész vagyunk.”");
    assert_eq!(TextRules::restore_accents("koszonom szepen a valaszt"), "köszönöm szépen a választ");
    assert_eq!(TextRules::restore_accents("I like the mar file"), "I like the mar file");
    assert_eq!(TextRules::apply_spoken_addresses("Írj az info kukac pelda pont hu címre."), "Írj az info@pelda.hu címre.");
    assert_eq!(TextRules::apply_spoken_addresses("A weboldal a pelda pont hu perjel blog."), "A weboldal a pelda.hu/blog.");
    assert_eq!(TextRules::apply_spoken_addresses("Send it to dorottya at example dot com."), "Send it to dorottya@example.com.");
}

#[test]
fn test_suffix_dictionary() {
    let d = eval_dictionary();
    assert_eq!(TextPipeline::apply_dictionary("a clickupban van", &d), "a ClickUpban van");
    assert_eq!(TextPipeline::apply_dictionary("küldd el Dorotyának", &d), "küldd el Dorottyának");
    assert_eq!(TextPipeline::apply_dictionary("a versel-en fut", &d), "a Vercel-en fut");
    assert_eq!(TextPipeline::apply_dictionary("nem szeretek verselni", &d), "nem szeretek verselni");
}

#[test]
fn test_acronyms_greeting_email() {
    assert_eq!(TextRules::apply_acronyms("nézd meg a pr-t és az api-t"), "nézd meg a PR-t és az API-t");
    assert_eq!(TextRules::apply_greeting_comma("szia meg tudnád nézni"), "szia, meg tudnád nézni");
    assert_eq!(TextRules::apply_greeting_comma("Szia Anna"), "Szia Anna");
    let mail = TextRules::layout_email("kedves péter köszönöm a választ üdvözlettel dorottya", DictationLanguage::Hu);
    assert_eq!(mail.as_deref(), Some("Kedves Péter!\n\nKöszönöm a választ.\n\nÜdvözlettel:\nDorottya"));
    assert_eq!(TextRules::layout_email("holnap jövök", DictationLanguage::Hu), None);
}

#[test]
fn test_context_fitting() {
    let cont = DictationContext { before: "Arra gondoltam, hogy ".into(), ..DictationContext::empty() };
    assert_eq!(TextRules::fit_to_context("Holnap elmehetnénk.", &cont, &[]), "holnap elmehetnénk.");
    let mid = DictationContext { before: "Megbeszéltem ".into(), after: " hogy jövő héten kezdünk.".into(), ..DictationContext::empty() };
    assert_eq!(TextRules::fit_to_context("Péterrel és Annával.", &mid, &[]), "Péterrel és Annával");
    let names = DictationContext { before: "Kedves Szőcs Gergely!\n\n".into(), ..DictationContext::empty() };
    assert_eq!(TextRules::apply_context_names("Köszönöm, Szőts úr.", &names), "Köszönöm, Szőcs úr.");
    let secure = DictationContext { before: "abc".into(), is_secure: true, ..DictationContext::empty() };
    assert_eq!(TextRules::fit_to_context("Szia.", &secure, &[]), "Szia.");
}

#[test]
fn test_browser_category() {
    assert_eq!(AppCategoryResolver::category_for(Some("com.google.Chrome"), Some("Beérkező levelek - Gmail")), AppCategory::Email);
    assert_eq!(AppCategoryResolver::category_for(Some("com.google.Chrome"), Some("general - Slack")), AppCategory::Work);
    assert_eq!(AppCategoryResolver::category_for(Some("com.google.Chrome"), Some("Hírek")), AppCategory::Other);
    assert_eq!(AppCategoryResolver::category_for(Some("com.apple.mail"), None), AppCategory::Email);
}

#[test]
fn test_llm_guard() {
    let raw = "mikor tudnál ránézni a kampányra";
    let local = "Mikor tudnál ránézni a kampányra?";
    assert!(LLMGuard::accept(raw, local, "Mikor tudnál ránézni a kampányra?"));
    assert!(!LLMGuard::accept(raw, local, "Holnap délután ránézek a kampányra, és küldök egy részletes összefoglalót mindenről."));
    assert!(!LLMGuard::accept(raw, local, "Mikor tudnál ránézni a kampányra."));
    assert!(!LLMGuard::accept(raw, local, ""));
}

#[test]
fn test_fillers_removed() {
    assert_eq!(TextPipeline::remove_fillers("Öö, szóval holnap hmm megyek."), "holnap megyek.");
    assert_eq!(TextPipeline::remove_fillers("I think, um, we should go."), "I think we should go.");
    assert_eq!(TextPipeline::remove_fillers("Ügyfél és önkormányzat"), "Ügyfél és önkormányzat");
    assert_eq!(TextPipeline::remove_fillers("Öhm, szóval holnap délelőtt, ööö, átküldöm az ajánlatot."), "holnap délelőtt átküldöm az ajánlatot.");
    assert_eq!(TextPipeline::remove_fillers("So, um, I think we should, uh, push it."), "I think we should push it.");
    assert_eq!(TextPipeline::remove_fillers("Hát izé, a számlát még nem kaptam meg."), "a számlát még nem kaptam meg.");
    assert_eq!(TextPipeline::remove_fillers("Szóval ez a terv."), "Szóval ez a terv.");
    assert_eq!(TextPipeline::remove_fillers("Ez, hmm, nagyon jó ötlet, ööö, csináljuk meg."), "Ez nagyon jó ötlet, csináljuk meg.");
}

#[test]
fn test_voice_dictionary_snippets_tones() {
    assert_eq!(TextPipeline::apply_voice_commands("Első pont új sor második pont"), "Első pont\nMásodik pont");
    assert_eq!(TextPipeline::apply_voice_commands("Hello. New paragraph thanks"), "Hello.\n\nThanks");
    let e = vec![DictionaryEntry::new("BusinessNative", &["biznisz nétív"]), DictionaryEntry::new("ClickUp", &[])];
    assert_eq!(TextPipeline::apply_dictionary("A biznisz nétív és a clickup.", &e), "A BusinessNative és a ClickUp.");
    let s = vec![Snippet::new("my calendly", "https://calendly.com/dorottya")];
    assert_eq!(TextPipeline::apply_snippets("My Calendly.", &s), "https://calendly.com/dorottya");
    assert_eq!(TextPipeline::apply_snippets("Itt a my calendly linkem", &s), "Itt a https://calendly.com/dorottya linkem");
    assert_eq!(TextPipeline::apply_tone("szia holnap jövök", StyleTone::Formal, &[]), "Szia holnap jövök.");
    assert_eq!(TextPipeline::apply_tone("Szia, holnap jövök.", StyleTone::Casual, &[]), "Szia, holnap jövök");
    assert_eq!(TextPipeline::apply_tone("Szia, holnap jövök. Jó lesz?", StyleTone::VeryCasual, &[]), "szia holnap jövök jó lesz?");
    assert_eq!(TextPipeline::apply_tone("Az API kész.", StyleTone::VeryCasual, &[]), "az API kész");
    assert_eq!(TextPipeline::apply_tone("Dorottya jön.", StyleTone::VeryCasual, &["Dorottya".into()]), "Dorottya jön");
    assert!(TextPipeline::needs_leading_space(Some('a'), "szia"));
    assert!(!TextPipeline::needs_leading_space(Some(' '), "szia"));
    assert!(!TextPipeline::needs_leading_space(Some('a'), ", szia"));
    assert!(!TextPipeline::needs_leading_space(None, "szia"));
}

#[test]
fn test_whole_snippet_and_full_pipeline() {
    let mut s = AppSettings::default();
    s.language = DictationLanguage::Auto;
    s.snippets = vec![Snippet::new("időpontfoglaló linkem", "https://cal.example.com/dorottya")];
    assert_eq!(TextPipeline::process("Az időpontfoglaló linkem.", &s, StyleTone::Formal), "https://cal.example.com/dorottya");
    let s = settings();
    let out = TextPipeline::process(
        "Öhm, szia anna, azt gondoltam hogy holnap tíz órakor, nem, inkább tizenegykor átküldöm a biznisz nétív ajánlatot",
        &s,
        StyleTone::Formal,
    );
    assert!(out == "Szia anna, azt gondoltam, hogy holnap tizenegykor átküldöm a BusinessNative ajánlatot." || out.contains("tizenegykor átküldöm a BusinessNative ajánlatot."), "{out}");
    assert!(!out.contains("tíz órakor"), "{out}");
}

#[test]
fn test_format_policy_skips_short_clean_sentences() {
    assert!(!FormatPolicy::needs_llm("Holnap jövök."));
    assert!(!FormatPolicy::needs_llm("Holnap délután átküldöm az ajánlatot neked."));
    assert!(!FormatPolicy::needs_llm("I will send the deck tomorrow morning."));
    assert!(FormatPolicy::needs_llm("Holnap kettőkor, vagyis háromkor találkozunk."));
    assert!(FormatPolicy::needs_llm("Let's meet at two, actually three."));
    assert!(FormatPolicy::needs_llm("Öhm holnap átküldöm az ajánlatot."));
    assert!(FormatPolicy::needs_llm("hogy hogy nem jött el senki tegnap este"));
    assert!(FormatPolicy::needs_llm(&"szó ".repeat(20)));
    assert!(FormatPolicy::needs_llm("holnap délután háromkor átküldöm neked az ajánlatot és a szerződést"));
}

struct SlowChat;
#[async_trait::async_trait]
impl ChatProvider for SlowChat {
    async fn complete(&self, _: &str, _: &str, _: Option<&str>) -> Result<String, FlowError> {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        Ok("X".into())
    }
}

#[tokio::test]
async fn test_formatter_timeout_falls_back_to_local() {
    let mut s = settings();
    s.llm_timeout_seconds = 0.5;
    let r = DictationFormatter::format("holnap átküldöm az ajánlatot neked", &s, &DictationContext::empty(), Some(&SlowChat)).await;
    assert!(!r.used_llm);
    assert_eq!(r.text, "Holnap átküldöm az ajánlatot neked.");
}

struct CountingChat(std::sync::atomic::AtomicUsize);
#[async_trait::async_trait]
impl ChatProvider for CountingChat {
    async fn complete(&self, _: &str, _: &str, _: Option<&str>) -> Result<String, FlowError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok("Ez egy elég hosszú megtisztított mondat a teszthez.".into())
    }
}

#[tokio::test]
async fn test_short_sentence_does_not_call_llm() {
    let chat = CountingChat(std::sync::atomic::AtomicUsize::new(0));
    let s = settings();
    let r = DictationFormatter::format("Holnap jövök.", &s, &DictationContext::empty(), Some(&chat)).await;
    assert!(!r.used_llm);
    assert_eq!(chat.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn test_audio_and_wav_and_env() {
    let d = encode_wav(&[0, 1, -1], 16000);
    assert_eq!(d.len(), 44 + 6);
    assert_eq!(&d[0..4], b"RIFF");
    assert_eq!(&d[8..12], b"WAVE");
    let env = "# c\nexport OPENAI_API_KEY=\"test-openai-key\"\nOTHER=1 # x\nEMPTY=\n";
    assert_eq!(audio::env_value(env, "OPENAI_API_KEY").as_deref(), Some("test-openai-key"));
    assert_eq!(audio::env_value(env, "OTHER").as_deref(), Some("1"));
    assert_eq!(audio::env_value(env, "EMPTY"), None);
    assert_eq!(audio::env_keys(env), vec!["OPENAI_API_KEY", "OTHER", "EMPTY"]);

    let mut x: u64 = 7;
    let noise: Vec<f32> = (0..32_000)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((x >> 40) as f32 / ((1u32 << 24) as f32) - 0.5) * 0.02
        })
        .collect();
    assert!(is_silent(&analyze(&noise, 16_000)));
    let mut speech = vec![0f32; 16_000];
    speech.extend((0..16_000).map(|i| (i as f64 * 0.2).sin() as f32 * 0.05));
    speech.extend(std::iter::repeat(0f32).take(16_000));
    let st = analyze(&speech, 16_000);
    assert!(!is_silent(&st));
    assert!(st.speech_seconds > 0.8 && st.speech_seconds < 1.2, "{}", st.speech_seconds);
    let prepared = prepare(&speech, 16_000);
    assert!(prepared.len() < speech.len());
    assert!(prepared.iter().map(|v| v.abs()).fold(0f32, f32::max) > 0.5);
    assert_eq!(clean_transcript("Köszönöm a figyelmet!", 0.01, Some(0.2)), "");
    assert_eq!(clean_transcript("Holnap jövök.", 0.3, Some(1.5)), "Holnap jövök.");
}

#[test]
fn test_clean_output_and_settings() {
    assert_eq!(Prompts::clean_output("„Szia!”"), "Szia!");
    assert_eq!(Prompts::clean_output("```\nhello\n```"), "hello");
    let s: AppSettings = serde_json::from_str(r#"{"chatModel":"gpt-4.1-mini"}"#).unwrap();
    assert_eq!(s.cleanup_provider, AIProvider::Openai);
    assert_eq!(s.transcription_provider, AIProvider::Openai);
    assert_eq!(s.chat_model, "gpt-4.1-mini");
    assert_eq!(s.language, DictationLanguage::Hu);
    assert!(!s.launch_at_login);
    let round: AppSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(round.chat_model, s.chat_model);
    assert_eq!(round.language, s.language);
}

#[test]
fn test_provider_metadata_and_requests() {
    assert!(!AIProvider::Anthropic.can_transcribe());
    assert_eq!(AIProvider::transcribers(), vec![AIProvider::Openai, AIProvider::Gemini]);
    assert_eq!(AIProvider::for_env_name("ANTHROPIC_API_KEY"), Some(AIProvider::Anthropic));
    assert_eq!(AIProvider::for_env_name("GEMINI_API_KEY"), Some(AIProvider::Gemini));
    assert_eq!(AIProvider::for_env_name("OPENAI_API_KEY"), Some(AIProvider::Openai));
    for p in AIProvider::all() {
        assert!(!p.chat_models().is_empty());
        assert!(p.key_page().starts_with("https://"));
    }
    let s = AppSettings::default();
    assert_eq!(s.anthropic_chat_model, AIProvider::Anthropic.chat_models()[0]);
    assert_eq!(s.gemini_transcription_model, AIProvider::Gemini.transcription_models()[0]);
    assert_eq!(s.transcription_model, "gpt-transcribe");
    assert_eq!(s.fallback_transcription_model, "gpt-4o-transcribe");
    assert_eq!(s.chat_model, "gpt-5.6-luna");
    assert_eq!(s.fallback_chat_model, "gpt-4.1-mini");

    let r = make_transcription_request("k", &[1], "gpt-transcribe", DictationLanguage::Auto, &["ClickUp".into()]);
    assert_eq!(r.url, "https://api.openai.com/v1/audio/transcriptions");
    let body = r.body_text();
    assert!(body.contains("name=\"languages[]\"\r\n\r\nhu"), "{body}");
    assert!(body.contains("name=\"languages[]\"\r\n\r\nen"));
    assert!(body.contains("name=\"keywords[]\"\r\n\r\nClickUp"));
    assert_eq!(r.header("Authorization"), Some("Bearer k"));

    let r = make_transcription_request("k", &[1], "gpt-4o-transcribe", DictationLanguage::Hu, &[]);
    let body = r.body_text();
    assert!(body.contains("name=\"language\"\r\n\r\nhu"), "{body}");
    assert!(!body.contains("languages[]"));

    assert_eq!(map_error(401, b""), FlowError::Unauthorized);
    assert_eq!(map_error(429, br#"{"error":{"code":"insufficient_quota"}}"#), FlowError::Quota);
    assert_eq!(map_error(429, b""), FlowError::RateLimit);
    assert_eq!(map_error(503, b""), FlowError::Server);
    assert_eq!(parse_chat(r#"{"choices":[{"message":{"content":" Kész "}}]}"#.as_bytes()).as_deref(), Some(" Kész "));

    let r = make_anthropic_request("test-key", "claude-haiku-5-5", "S", "U");
    assert_eq!(r.url, "https://api.anthropic.com/v1/messages");
    assert_eq!(r.header("x-api-key"), Some("test-key"));
    assert_eq!(r.header("anthropic-version"), Some("2023-06-01"));
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(v["system"], "S");
    assert_eq!(v["model"], "claude-haiku-5-5");
    assert_eq!(parse_anthropic(r#"{"content":[{"type":"thinking","thinking":"x"},{"type":"text","text":"Tisztított."}]}"#.as_bytes()).as_deref(), Some("Tisztított."));

    let r = make_gemini_transcription_request("g-key", "gemini-3.5-flash-lite", &[1, 2, 3], DictationLanguage::Hu, &["Business Flow".into()]);
    assert_eq!(r.url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.5-flash-lite:generateContent");
    assert_eq!(r.header("x-goog-api-key"), Some("g-key"));
    assert!(!r.url.contains("g-key"));
    let body = r.body_text();
    assert!(body.contains("audio/wav"), "{body}");
    assert!(body.contains("AQID"));
    assert!(body.contains("Business Flow"));
    assert_eq!(parse_gemini(br#"{"candidates":[{"content":{"parts":[{"text":"szia vilag"}]}}]}"#).as_deref(), Some("szia vilag"));
    assert_eq!(
        parse_gemini(r#"{"candidates":[{"content":{"parts":[{"text":"gondol","thought":true},{"text":"válasz"}]}}]}"#.as_bytes()).as_deref(),
        Some("válasz")
    );

    assert_eq!(make_key_test_request(AIProvider::Openai, "k").header("Authorization"), Some("Bearer k"));
    assert_eq!(make_key_test_request(AIProvider::Anthropic, "k").url, "https://api.anthropic.com/v1/models");
    assert_eq!(make_key_test_request(AIProvider::Gemini, "k").header("x-goog-api-key"), Some("k"));
}

struct Scripted {
    handler: Mutex<Box<dyn FnMut(&BuiltRequest) -> Result<Vec<u8>, FlowError> + Send>>,
    seen: Mutex<Vec<BuiltRequest>>,
}

#[async_trait::async_trait]
impl HttpClient for Scripted {
    async fn send(&self, req: BuiltRequest) -> Result<Vec<u8>, FlowError> {
        self.seen.lock().unwrap().push(req.clone());
        (self.handler.lock().unwrap())(&req)
    }
}

fn scripted(f: impl FnMut(&BuiltRequest) -> Result<Vec<u8>, FlowError> + Send + 'static) -> AIRouter<Scripted> {
    let mut router = AIRouter::new(Scripted { handler: Mutex::new(Box::new(f)), seen: Mutex::new(vec![]) });
    router.retry_delay = std::time::Duration::ZERO;
    router
}

#[tokio::test]
async fn test_router_fallback() {
    let gemini_ok = br#"{"candidates":[{"content":{"parts":[{"text":"szia vilag"}]}}]}"#.to_vec();
    let claude_ok = r#"{"content":[{"type":"thinking","thinking":"x"},{"type":"text","text":"Tisztított."}]}"#.as_bytes().to_vec();
    let openai_ok = r#"{"choices":[{"message":{"content":"OpenAI kész."}}]}"#.as_bytes().to_vec();

    let router = scripted({
        let gemini_ok = gemini_ok.clone();
        move |_| Ok(gemini_ok.clone())
    });
    let mut s = AppSettings::default();
    s.transcription_provider = AIProvider::Gemini;
    let (r, p) = router.transcribe(&s, &ProviderKeys::new(&[(AIProvider::Gemini, "g")]), &[0], &[]).await;
    assert_eq!(r.unwrap(), "szia vilag");
    assert_eq!(p, Some(AIProvider::Gemini));

    let router = scripted({
        let gemini_ok = gemini_ok.clone();
        move |req| {
            if req.url.contains("api.openai.com") {
                Err(FlowError::Server)
            } else {
                Ok(gemini_ok.clone())
            }
        }
    });
    let (r, p) = router.transcribe(&AppSettings::default(), &ProviderKeys::new(&[(AIProvider::Openai, "o"), (AIProvider::Gemini, "g")]), &[0], &[]).await;
    assert_eq!(r.unwrap(), "szia vilag");
    assert_eq!(p, Some(AIProvider::Gemini));
    assert!(router.http.seen.lock().unwrap().iter().any(|r| r.url.contains("api.openai.com")));

    let router = scripted(|_| Ok(vec![]));
    let mut s = AppSettings::default();
    s.transcription_provider = AIProvider::Anthropic;
    let (r, _) = router.transcribe(&s, &ProviderKeys::new(&[(AIProvider::Anthropic, "a")]), &[0], &[]).await;
    assert_eq!(r, Err(FlowError::NoApiKey));
    assert!(!ProviderKeys::new(&[(AIProvider::Anthropic, "a")]).can_transcribe());
    assert!(router.http.seen.lock().unwrap().is_empty());

    let mut s = AppSettings::default();
    s.cleanup_provider = AIProvider::Anthropic;
    let router = scripted({
        let claude_ok = claude_ok.clone();
        move |_| Ok(claude_ok.clone())
    });
    let ok = router.complete(&s, &ProviderKeys::new(&[(AIProvider::Anthropic, "a"), (AIProvider::Openai, "o")]), "s", "u", None).await.unwrap();
    assert_eq!(ok, "Tisztított.");

    let router = scripted({
        let openai_ok = openai_ok.clone();
        move |req| if req.url.contains("api.anthropic.com") { Err(FlowError::Unauthorized) } else { Ok(openai_ok.clone()) }
    });
    let fb = router.complete(&s, &ProviderKeys::new(&[(AIProvider::Anthropic, "a"), (AIProvider::Openai, "o")]), "s", "u", None).await.unwrap();
    assert_eq!(fb, "OpenAI kész.");
    assert!(router.http.seen.lock().unwrap().first().unwrap().url.contains("api.anthropic.com"));

    s.cleanup_provider = AIProvider::Gemini;
    assert_eq!(
        chat_order(&s, &ProviderKeys::new(&[(AIProvider::Openai, "o"), (AIProvider::Gemini, "g"), (AIProvider::Anthropic, "a")])),
        vec![AIProvider::Gemini, AIProvider::Openai, AIProvider::Anthropic]
    );
    assert!(chat_order(&s, &ProviderKeys::new(&[])).is_empty());
}

#[test]
fn test_hotkey_engine() {
    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    assert_eq!(e.modifiers_changed(mods(&[ModifierKey::Fn]), 0.0), vec![HotkeyAction::Start { kind: hotkey::DictationKind::Dictation, hands_free: false }]);
    assert_eq!(e.modifiers_changed(mods(&[]), 1.0), vec![HotkeyAction::Stop]);

    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    let _ = e.modifiers_changed(mods(&[ModifierKey::Fn]), 0.0);
    assert_eq!(e.modifiers_changed(mods(&[]), 0.05), vec![HotkeyAction::Cancel]);

    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    let _ = e.modifiers_changed(mods(&[ModifierKey::Fn]), 0.0);
    let (actions, consume) = e.key_down(49, false, 0.1);
    assert_eq!(actions, vec![HotkeyAction::LockHandsFree]);
    assert!(consume);
    assert!(e.key_up(49));
    assert!(e.modifiers_changed(mods(&[]), 0.3).is_empty());
    assert_eq!(e.modifiers_changed(mods(&[ModifierKey::Fn]), 5.0), vec![HotkeyAction::Stop]);
    assert_eq!(e.state, HotkeyState::Idle);

    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    let _ = e.modifiers_changed(mods(&[ModifierKey::Fn]), 0.0);
    assert_eq!(e.modifiers_changed(mods(&[ModifierKey::Fn, ModifierKey::LeftControl]), 0.05), vec![HotkeyAction::SwitchKind(hotkey::DictationKind::Command)]);
    assert_eq!(e.modifiers_changed(mods(&[]), 1.0), vec![HotkeyAction::Stop]);

    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    let _ = e.modifiers_changed(mods(&[ModifierKey::Fn]), 0.0);
    let (actions, consume) = e.key_down(123, false, 0.1);
    assert_eq!(actions, vec![HotkeyAction::Cancel]);
    assert!(!consume);

    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    assert!(!e.key_down(53, false, 0.0).1);
    let _ = e.modifiers_changed(mods(&[ModifierKey::Fn]), 0.0);
    let _ = e.key_down(49, false, 0.1);
    assert_eq!(e.key_down(53, false, 1.0).0, vec![HotkeyAction::Cancel]);

    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    let _ = e.modifiers_changed(mods(&[ModifierKey::LeftControl, ModifierKey::LeftCommand]), 0.0);
    let (actions, consume) = e.key_down(9, false, 0.1);
    assert_eq!(actions, vec![HotkeyAction::PasteLast]);
    assert!(consume);

    let mut s = ShortcutSettings::default();
    s.push_to_talk.push(ShortcutTrigger::Mouse { button: 4 });
    let mut e = HotkeyEngine::new(s);
    assert_eq!(e.mouse_down(4, 0.0).0, vec![HotkeyAction::Start { kind: hotkey::DictationKind::Dictation, hands_free: false }]);
    assert_eq!(e.mouse_up(4, 2.0).0, vec![HotkeyAction::Stop]);
    assert!(!e.mouse_down(3, 3.0).1);

    let mut e = HotkeyEngine::new(ShortcutSettings::default());
    assert_eq!(e.bar_clicked(), vec![HotkeyAction::Start { kind: hotkey::DictationKind::Dictation, hands_free: true }]);
    assert_eq!(e.bar_clicked(), vec![HotkeyAction::Stop]);
}
