use crate::models::{AppCategory, AppSettings, DictationContext, DictationLanguage, DictionaryEntry, Snippet, StyleTone};
use crate::text_rules::{AppCategoryResolver, TextRules};
use crate::util::{cap_first, fold_diacritics, group, regex, replace_all, replace_literal, replace_mapped, trim_punct_ws, words};
use regex::escape;

pub struct TextPipeline;

impl TextPipeline {
    pub fn remove_fillers(text: &str) -> String {
        let mark = "\u{1}";
        let filler = format!(r"(?i)(?<![\p{{L}}\p{{N}}])(ö+h*m*|e+h*m+|hm+|mhm|u+h*m*|u+m+|e+r+m+|ööö+|izé|izéé+)(?![\p{{L}}\p{{N}}])[,.…]*\s*");
        let mut s = replace_mapped(text, &regex(&filler), |m| {
            let chunk = group(m, 0);
            Some(if chunk.contains(',') { format!("{mark},") } else { mark.to_string() })
        });
        if !s.contains(mark) {
            return tidy(&s);
        }
        let discourse = r"(?:szóval|hát|nos|na|so|well|okay|oké|like)";
        s = replace_all(&s, &format!(r"(?i)^({m}[,\s]*)*{d}[,\s]*{m}", m = mark, d = discourse), mark);
        s = replace_all(&s, &format!(r"(?i)^{m}[,\s]*{d}(?:[,\s]+|$)", m = mark, d = discourse), mark);
        let re3 = regex(&format!(r"(,?)\s*{m}([,.…\s{m}]*)", m = mark));
        let orig = s.clone();
        s = replace_mapped(&orig, &re3, |m| {
            let start = m.get(0).unwrap().start();
            let before = &orig[..start];
            let g1 = group(m, 1);
            let g2 = group(m, 2);
            let had_both = !g1.is_empty() && g2.contains(',');
            let chunk = before.split(|c| ",.;:!?\n".contains(c)).last().unwrap_or("");
            let ws = words(chunk);
            let prev = ws.last().cloned().unwrap_or_default();
            if had_both && ws.len() >= 3 && !NO_COMMA_AFTER.iter().any(|w| *w == prev) {
                return Some(", ".into());
            }
            Some(" ".into())
        });
        tidy(&s)
    }

    pub fn apply_spoken_punctuation(text: &str) -> String {
        let rules = [
            (r"(?i)\s*[,.]?\s*\b(kérdőjel|question mark)\b[,.]?", "?"),
            (r"(?i)\s*[,.]?\s*\b(felkiáltójel|exclamation (?:mark|point))\b[,.]?", "!"),
            (r"(?i)\s*[,.]?\s*\b(pontosvessző|semicolon)\b[,.]?", ";"),
            (r"(?i)\s*[,.]?\s*\b(kettőspont)\b[,.]?", ":"),
            (r"(?i)\s*,?\s*\b(vessző|comma)\b,?", ","),
        ];
        let mut s = text.to_string();
        let mut hit = false;
        for (p, r) in rules {
            if regex(p).is_match(&s).unwrap_or(false) {
                s = replace_all(&s, p, r);
                hit = true;
            }
        }
        if !hit {
            return text.to_string();
        }
        s = replace_all(&s, r"([?!:;,])(?=\p{L})", "$1 ");
        s = replace_all(&s, r"^[\s?!:;,]+", "");
        let mut out = String::new();
        let mut cap = false;
        for ch in s.chars() {
            if cap && ch.is_alphabetic() {
                out.extend(ch.to_uppercase());
                cap = false;
                continue;
            }
            if ch == '?' || ch == '!' {
                cap = true;
            } else if !ch.is_whitespace() {
                cap = false;
            }
            out.push(ch);
        }
        replace_all(&out, r"([?!:;,])\1+", "$1")
    }

    pub fn apply_voice_commands(text: &str) -> String {
        let mut s = text.to_string();
        let commands = [
            (r"(?i),?\s*\b(új bekezdés|new paragraph)\b[,.]?\s*", "\n\n"),
            (r"(?i),?\s*\b(új sor|new line)\b[,.]?\s*", "\n"),
        ];
        for (p, r) in commands {
            s = replace_all(&s, p, r);
        }
        s = Self::apply_spoken_punctuation(&s);
        s = capitalize_after_newlines(&s);
        s.trim().to_string()
    }

    pub fn apply_dictionary(text: &str, entries: &[DictionaryEntry]) -> String {
        let mut s = text.to_string();
        let suffix = suffix_group();
        for e in entries {
            let term = e.term.trim();
            if term.is_empty() {
                continue;
            }
            let mut variants: Vec<String> = e.heard_as.iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
            variants.push(term.to_string());
            let suffix_ok = term.chars().count() >= 4 && term.chars().last().map(|c| c.is_alphabetic()).unwrap_or(false);
            for v in variants {
                if suffix_ok {
                    let lv = v.chars().last();
                    let lt = term.chars().last();
                    if let (Some(lv), Some(lt)) = (lv, lt) {
                        if "aeAE".contains(lv) && "aeAE".contains(lt) {
                            let stem_v: String = v.chars().take(v.chars().count() - 1).collect();
                            let stem_t: String = term.chars().take(term.chars().count() - 1).collect();
                            let p = format!(r"(?i)(?<![\p{{L}}\p{{N}}]){}([áé]){}(?![\p{{L}}\p{{N}}])", escape(&stem_v), suffix);
                            let stem_t2 = stem_t.clone();
                            s = replace_mapped(&s, &regex(&p), |c| {
                                Some(format!("{}{}{}", stem_t2, group(c, 1).to_lowercase(), group(c, 2)))
                            });
                        }
                    }
                    let p = format!(r"(?i)(?<![\p{{L}}\p{{N}}]){}{}(?![\p{{L}}\p{{N}}])", escape(&v), suffix);
                    let term_owned = term.to_string();
                    s = replace_mapped(&s, &regex(&p), |c| Some(format!("{term_owned}{}", group(c, 1))));
                } else {
                    let p = format!(r"(?i)(?<![\p{{L}}\p{{N}}]){}(?![\p{{L}}\p{{N}}])", escape(&v));
                    s = replace_mapped(&s, &regex(&p), |_| Some(term.to_string()));
                }
            }
        }
        s
    }

    pub fn apply_snippets(text: &str, snippets: &[Snippet]) -> String {
        let mut s = text.to_string();
        for sn in snippets {
            let trig = sn.trigger.trim();
            if trig.is_empty() {
                continue;
            }
            let mut bare = trim_punct_ws(&s);
            bare = replace_all(&bare, r"(?i)^(az én|a|az|the|my|egy)\s+", "");
            if fold_diacritics(&bare) == fold_diacritics(trig) {
                return sn.expansion.clone();
            }
            let parts: Vec<String> = trig.split_whitespace().map(|w| escape(w)).collect();
            let p = format!(r"(?i)(?<![\p{{L}}\p{{N}}]){}(?![\p{{L}}\p{{N}}])[.,!?]?", parts.join(r"[\s,-]*"));
            s = replace_literal(&s, &p, &sn.expansion);
        }
        s
    }

    pub fn apply_tone(text: &str, tone: StyleTone, protected_terms: &[String]) -> String {
        let mut s = text.trim().to_string();
        if s.is_empty() {
            return s;
        }
        match tone {
            StyleTone::Formal => {
                s = cap_first(&s);
                if s.chars().last().map(|c| c.is_alphabetic() || c.is_numeric()).unwrap_or(false) {
                    s.push('.');
                }
            }
            StyleTone::Casual => {
                s = cap_first(&s);
                if s.ends_with('.') && !s.ends_with("...") {
                    s.pop();
                }
            }
            StyleTone::VeryCasual => {
                s = lowercase_sentence_starts(&s, protected_terms);
                if s.ends_with('.') && !s.ends_with("...") {
                    s.pop();
                }
                s = replace_all(&s, r"(?<=[\p{L}\p{N}]),(?=\s)", "");
                s = replace_all(&s, r"\.(\s+)", "$1");
            }
        }
        s
    }

    pub fn needs_leading_space(previous: Option<char>, text: &str) -> bool {
        let Some(p) = previous else { return false };
        let Some(first) = text.chars().next() else { return false };
        if p.is_whitespace() {
            return false;
        }
        if "([{\"'„«/-".contains(p) {
            return false;
        }
        if ".,!?;:)]}…".contains(first) {
            return false;
        }
        true
    }

    pub fn process(raw: &str, settings: &AppSettings, tone: StyleTone) -> String {
        Self::process_with(raw, settings, tone, &DictationContext::empty(), AppCategory::Other)
    }

    pub fn process_with(raw: &str, settings: &AppSettings, tone: StyleTone, context: &DictationContext, category: AppCategory) -> String {
        let mut s = raw.trim().to_string();
        if s.is_empty() {
            return s;
        }
        let protected: Vec<String> = settings.dictionary.iter().map(|d| d.term.clone()).collect();
        if settings.remove_fillers {
            s = Self::remove_fillers(&s);
        }
        if settings.voice_commands {
            s = Self::apply_voice_commands(&s);
        }
        s = TextRules::apply_backtrack(&s);
        s = Self::apply_dictionary(&s, &settings.dictionary);
        s = Self::apply_snippets(&s, &settings.snippets);
        if settings.snippets.iter().any(|sn| sn.expansion == s) {
            return s;
        }
        s = TextRules::apply_spoken_addresses(&s);
        s = TextRules::restore_accents(&s);
        let lang = TextRules::language(&s);
        let unknown = matches!(lang, DictationLanguage::Auto);
        if matches!(lang, DictationLanguage::Hu) || (unknown && matches!(settings.language, DictationLanguage::Hu)) {
            s = TextRules::apply_hungarian_commas(&s);
            s = TextRules::apply_hungarian_quotes(&s);
        }
        s = TextRules::apply_acronyms(&s);
        if settings.voice_commands {
            s = TextRules::apply_lists(&s);
        }
        if category == AppCategory::Email {
            if let Some(mail) = TextRules::layout_email(&s, if matches!(lang, DictationLanguage::En) { DictationLanguage::En } else { DictationLanguage::Hu }) {
                s = mail;
            } else {
                s = finish_prose(&s, tone, &protected);
            }
        } else {
            s = finish_prose(&s, tone, &protected);
        }
        s = replace_all(&s, r"[ \t]{2,}", " ");
        s = replace_all(&s, r",{2,}", ",");
        s = TextRules::apply_context_names(&s, context);
        s = TextRules::fit_to_context(&s, context, &protected);
        s
    }
}

fn finish_prose(s: &str, tone: StyleTone, protected: &[String]) -> String {
    let mut s = TextRules::apply_greeting_comma(s);
    if tone != StyleTone::VeryCasual {
        s = TextRules::apply_question_mark(&s);
    }
    if TextRules::is_list(&s) && tone != StyleTone::VeryCasual {
        cap_first(s.trim())
    } else {
        TextPipeline::apply_tone(&s, tone, protected)
    }
}

const NO_COMMA_AFTER: &[&str] = &["should", "would", "could", "can", "will", "must", "to", "the", "a", "an", "of", "and", "is", "are", "kell", "fog", "fogok", "fogom", "akarok", "az", "egy", "és", "hogy", "meg", "el", "nem"];

fn tidy(text: &str) -> String {
    let s = replace_all(text, r"\s{2,}", " ");
    let s = replace_all(&s, r"\s+([,.!?;:])", "$1");
    let s = replace_all(&s, r"^[,;:\s]+", "");
    s.trim().to_string()
}

fn capitalize_after_newlines(s: &str) -> String {
    let mut out = String::new();
    let mut cap = false;
    for ch in s.chars() {
        if ch == '\n' {
            cap = true;
            out.push(ch);
            continue;
        }
        if cap && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            cap = false;
            continue;
        }
        if cap && !ch.is_whitespace() {
            cap = false;
        }
        out.push(ch);
    }
    out
}

fn lowercase_sentence_starts(s: &str, protected: &[String]) -> String {
    let words: Vec<&str> = s.split(' ').collect();
    let mut out = Vec::new();
    let mut start = true;
    for w in words {
        let mut word = w.to_string();
        if start {
            if let Some(f) = word.chars().next() {
                if f.is_uppercase() {
                    let core = word.trim_matches(|c: char| c.is_ascii_punctuation() || "„”".contains(c)).to_string();
                    let is_acronym = core.chars().count() > 1 && core.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()) && core.chars().any(|c| c.is_alphabetic());
                    let is_protected = protected.iter().any(|p| p.eq_ignore_ascii_case(&core));
                    if !is_acronym && !is_protected {
                        let lower: String = f.to_lowercase().collect();
                        word = format!("{lower}{}", &word[f.len_utf8()..]);
                    }
                }
            }
        }
        if !word.is_empty() {
            start = word.chars().last().map(|c| ".!?\n".contains(c)).unwrap_or(false);
        }
        out.push(word);
    }
    out.join(" ")
}

fn suffix_group() -> String {
    let suffixes = [
        "jából", "jéből", "jának", "jének", "ként", "ból", "ből", "ról", "ről", "tól", "től", "nak", "nek", "val", "vel", "hoz", "hez", "höz", "nál", "nél", "ban", "ben", "ért", "ig", "ba", "be", "ra", "re", "on", "en", "ön", "ot", "et", "öt", "at", "ék", "os", "es", "ös", "ja", "je", "é", "t", "n", "s",
    ];
    format!(r"(-\p{{Ll}}{{1,12}}|{})?", suffixes.join("|"))
}

impl AppCategoryResolver {
    pub fn from_context(ctx: &DictationContext) -> AppCategory {
        Self::category_for(ctx.bundle_id.as_deref(), ctx.window_title.as_deref())
    }
}
