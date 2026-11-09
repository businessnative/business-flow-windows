use crate::models::{AppCategory, DictationContext};
use crate::util::{cap_first, fold_diacritics, group, is_capitalized, lev, regex, replace_all, replace_mapped, trim_punct_ws, words};
use fancy_regex::Regex;
use std::sync::LazyLock;

fn has(list: &[&str], item: &str) -> bool {
    list.iter().any(|x| *x == item)
}

fn accent_of(w: &str) -> Option<&'static str> {
    ACCENTS.iter().find(|(k, _)| *k == w).map(|(_, v)| *v)
}

pub struct TextRules;

impl TextRules {
    pub fn language(s: &str) -> crate::models::DictationLanguage {
        let lower = s.to_lowercase();
        let mut hu = 0i32;
        let mut en = 0i32;
        if lower.chars().any(|c| "őű".contains(c)) {
            hu += 3;
        }
        if lower.chars().any(|c| "áéíóöúü".contains(c)) {
            hu += 2;
        }
        for w in words(&lower) {
            if has(HU_WORDS, w.as_str()) {
                hu += 1;
            }
            if has(EN_WORDS, w.as_str()) {
                en += 1;
            }
        }
        if hu > en {
            crate::models::DictationLanguage::Hu
        } else if en > hu {
            crate::models::DictationLanguage::En
        } else {
            crate::models::DictationLanguage::Auto
        }
    }

    pub fn apply_backtrack(text: &str) -> String {
        let mut s = remove_scratched_sentences(text);
        for _ in 0..5 {
            let Some(m) = BACKTRACK_RE.find(&s).ok().flatten() else { break };
            let head = &s[..m.start()];
            let tail = &s[m.end()..];
            let mut boundary = 0usize;
            if let Some(end) = SENTENCE_BOUNDARY_RE.find_iter(head).filter_map(|r| r.ok()).map(|m| m.end()).last() {
                boundary = end;
            }
            let prior = &head[..boundary];
            let mut sentence: Vec<String> = head[boundary..].split(' ').filter(|w| !w.is_empty()).map(|w| w.to_string()).collect();
            let y0 = tail.split(' ').next().unwrap_or("");
            if sentence.is_empty() || y0.is_empty() {
                break;
            }
            let mut drop_at = sentence.len() - 1;
            if is_number_word(y0) {
                if let Some(r) = sentence.iter().rposition(|w| is_number_word(w)) {
                    let mut k = r;
                    while k > 0 && is_number_word(&sentence[k - 1]) {
                        k -= 1;
                    }
                    drop_at = k;
                }
            } else if is_capitalized(y0) {
                if let Some(r) = sentence.iter().enumerate().rposition(|(i, w)| i > 0 && is_capitalized(w)) {
                    drop_at = r;
                }
            } else {
                let trimmed = y0.to_lowercase().trim_matches(|c: char| c.is_ascii_punctuation() || "„”".contains(c)).to_string();
                let suf: String = trimmed.chars().rev().take(3).collect::<String>().chars().rev().collect();
                if suf.chars().count() == 3 {
                    if let Some(r) = sentence.iter().enumerate().rposition(|(i, w)| i > 0 && w.to_lowercase().ends_with(&suf)) {
                        drop_at = r;
                    }
                }
            }
            if drop_at == 0 && sentence.len() > 1 {
                drop_at = sentence.len() - 1;
            }
            sentence.truncate(drop_at);
            let fixed_tail = if sentence.is_empty() { cap_first(tail) } else { tail.to_string() };
            let rebuilt = if sentence.is_empty() {
                fixed_tail
            } else {
                format!("{} {}", sentence.join(" "), fixed_tail)
            };
            s = format!("{prior}{rebuilt}");
        }
        s
    }

    pub fn apply_lists(text: &str) -> String {
        if text.contains('\n') {
            return text.to_string();
        }
        let ms: Vec<_> = ORDINAL_RE.captures_iter(text).filter_map(|c| c.ok()).collect();
        let mut seq: Vec<(usize, usize, usize)> = Vec::new();
        for m in &ms {
            let word = group(m, 1);
            let Some(k) = ordinal_index(word) else { continue };
            let start = m.get(0).unwrap().start();
            let end = m.get(0).unwrap().end();
            if k == seq.len() {
                if k == 0 {
                    let prev = words(&text[..start]).last().cloned().unwrap_or_default();
                    if has(ARTICLES, prev.as_str()) {
                        continue;
                    }
                }
                seq.push((start, end, k));
            } else if k == 0 {
                seq.clear();
                seq.push((start, end, k));
            }
        }
        if seq.len() < 2 {
            return text.to_string();
        }
        let mut intro = text[..seq[0].0].trim().to_string();
        intro = replace_all(&intro, r"[,;:\s]+$", "");
        let mut items = Vec::new();
        for (i, (_start, end, _)) in seq.iter().enumerate() {
            let item_end = if i + 1 < seq.len() { seq[i + 1].0 } else { text.len() };
            let mut it = text[*end..item_end].trim().to_string();
            it = replace_all(&it, r"(?i)[\s,;]+(és|and|meg|valamint|and then|aztán)$", "");
            it = it.trim_matches(|c: char| " ,;:.".contains(c) || c.is_whitespace()).to_string();
            it = replace_all(&it, r"(?i)^(the|a|an|az)\s+(?=\p{L})", "");
            if it.is_empty() {
                return text.to_string();
            }
            items.push(cap_first(&it));
        }
        let body = items.iter().enumerate().map(|(i, it)| format!("{}. {}", i + 1, it)).collect::<Vec<_>>().join("\n");
        if intro.is_empty() { body } else { format!("{intro}:\n{body}") }
    }

    pub fn is_list(s: &str) -> bool {
        s.contains('\n') && regex(r"(?m)^\d+\. ").is_match(s).unwrap_or(false)
    }

    pub fn apply_hungarian_commas(text: &str) -> String {
        let mut s = replace_mapped(text, &HU_CONJ_RE, |m| {
            let start = m.get(0).unwrap().start();
            let before = &text[..start];
            if before.chars().last().map(|c| ",;:".contains(c)).unwrap_or(false) {
                return None;
            }
            let prev = words(before).last().cloned().unwrap_or_default();
            if has(HU_NO_COMMA_BEFORE, prev.as_str()) {
                return None;
            }
            Some(format!(",{}{}", group(m, 1), group(m, 2)))
        });
        let rel = regex(&format!(
            r"(?:^|(?<=[.!?]\s))((?:\p{{L}}+\s){{1,2}}\p{{L}}+, (?:{})(?:\s[\p{{L}}\p{{N}}]+){{2,5}}?)\s({})(?![\p{{L}}])",
            RELATIVE_STARTS.join("|"),
            MAIN_RESUMERS.join("|")
        ));
        // The closure must see the string as it is before this pass.
        let current = s.clone();
        s = replace_mapped(&current, &rel, |m| {
            let clause = group(m, 1);
            if clause.contains(',') && clause.split(',').count() > 2 {
                return None;
            }
            Some(format!("{clause}, {}", group(m, 2)))
        });
        s
    }

    pub fn is_question(sentence: &str) -> bool {
        let mut w = words(sentence);
        while w.first().map(|f| has(GREETINGS, f.as_str())).unwrap_or(false) {
            w.remove(0);
        }
        let Some(first) = w.first().cloned() else { return false };
        if regex(r"(?<=\p{L})-e(?![\p{L}])").is_match(sentence).unwrap_or(false) {
            return true;
        }
        if w.last().map(|s| s.as_str()) == Some("ugye") {
            return true;
        }
        if has(HU_QUESTION_STARTS, first.as_str()) && first != "van" && first != "nincs" {
            return true;
        }
        if (first == "van" || first == "nincs") && w.len() <= 6 && sentence.to_lowercase().contains("valami") {
            return true;
        }
        if w.iter().take(3).any(|x| has(HU_POLITE_QUESTION, x.as_str())) {
            return true;
        }
        if has(EN_QUESTION_STARTS, first.as_str()) && !matches!(Self::language(sentence), crate::models::DictationLanguage::Hu) {
            return true;
        }
        false
    }

    pub fn apply_question_mark(text: &str) -> String {
        let Some(last) = text.chars().last() else { return text.to_string() };
        if !(last.is_alphabetic() || last.is_numeric()) {
            return text.to_string();
        }
        let tail = text.rsplit(|c| ".!?\n".contains(c)).next().unwrap_or(text);
        if Self::is_question(tail) { format!("{text}?") } else { text.to_string() }
    }

    pub fn apply_hungarian_quotes(text: &str) -> String {
        let s = replace_all(text, r#""([^"\n]+)""#, "„$1”");
        replace_all(&s, r"“([^”\n]+)”", "„$1”")
    }

    pub fn restore_accents(text: &str) -> String {
        if text.chars().any(|c| "áéíóöőúüűÁÉÍÓÖŐÚÜŰ".contains(c)) {
            return text.to_string();
        }
        let ws = words(text);
        let hits = ws.iter().filter(|w| accent_of(w).unwrap_or(w.as_str()) != w.as_str()).count();
        let lang = Self::language(text);
        if !(hits >= 2 || (hits >= 1 && matches!(lang, crate::models::DictationLanguage::Hu))) {
            return text.to_string();
        }
        if matches!(lang, crate::models::DictationLanguage::En) {
            return text.to_string();
        }
        replace_mapped(text, &WORD_RE, |m| {
            let w = group(m, 0);
            let rep = accent_of(&w.to_lowercase())?;
            if rep == w.to_lowercase() {
                return None;
            }
            if w == "mar" {
                return None;
            }
            if w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                Some(cap_first(rep))
            } else {
                Some(rep.to_string())
            }
        })
    }

    pub fn apply_spoken_addresses(text: &str) -> String {
        let s = replace_mapped(text, &EMAIL_RE, |m| {
            Some(format!("{}@{}", group(m, 1).to_lowercase(), collapse_domain(group(m, 2))))
        });
        replace_mapped(&s, &DOMAIN_RE, |m| {
            let path = replace_all(group(m, 2), r"(?i)\s+(?:perjel|per|slash)\s+", "/");
            Some(format!("{}{}", collapse_domain(group(m, 1)), path.to_lowercase()))
        })
    }

    pub fn apply_acronyms(text: &str) -> String {
        replace_mapped(text, &ACRONYM_RE, |m| Some(group(m, 0).to_uppercase()))
    }

    pub fn apply_greeting_comma(text: &str) -> String {
        replace_mapped(text, &GREETING_COMMA_RE, |m| Some(format!("{}, ", group(m, 1))))
    }

    pub fn layout_email(text: &str, lang: crate::models::DictationLanguage) -> Option<String> {
        if text.contains('\n') {
            return None;
        }
        let mut body = text.trim().to_string();
        let mut greeting = None;
        if let Some(m) = MAIL_GREETING_RE.captures(&body).ok().flatten() {
            let g = group(&m, 1);
            let n1 = group(&m, 2);
            let n2 = group(&m, 3).trim().to_string();
            let strong = ["kedves", "tisztelt", "dear"].contains(&g.to_lowercase().as_str());
            let n1l = n1.to_lowercase();
            let ok = is_capitalized(n1)
                || (strong && !has(COMMON_NON_NAMES, n1l.as_str()))
                || (!has(COMMON_NON_NAMES, n1l.as_str()) && !has(HU_WORDS, n1l.as_str()));
            if ok {
                let mut name_parts = vec![cap_first(n1)];
                if !n2.is_empty() {
                    name_parts.push(n2);
                }
                let name = name_parts.join(" ");
                let is_en = ["dear", "hi", "hello"].contains(&g.to_lowercase().as_str()) && matches!(lang, crate::models::DictationLanguage::En);
                let punct = if is_en { "," } else { "!" };
                greeting = Some(format!("{} {name}{punct}", cap_first(&g.to_lowercase())));
                let end = m.get(0).unwrap().end();
                body = body[end..].to_string();
            }
        }
        let mut signoff = None;
        let sign = STRONG_SIGNOFF_RE.captures(&body).ok().flatten().or_else(|| WEAK_SIGNOFF_RE.captures(&body).ok().flatten());
        if let Some(m) = sign {
            let so = group(&m, 1);
            let name = group(&m, 2);
            if !has(COMMON_NON_NAMES, name.to_lowercase().as_str()) {
                let punct = if matches!(lang, crate::models::DictationLanguage::En) { "," } else { ":" };
                let pretty = name.split_whitespace().map(cap_first).collect::<Vec<_>>().join(" ");
                signoff = Some(format!("{}{punct}\n{pretty}", cap_first(&so.to_lowercase())));
                let start = m.get(0).unwrap().start();
                body = body[..start].to_string();
            }
        }
        if greeting.is_none() && signoff.is_none() {
            return None;
        }
        body = cap_first(body.trim_matches(|c: char| c.is_whitespace() || c == ','));
        if body.chars().last().map(|c| c.is_alphabetic() || c.is_numeric()).unwrap_or(false) {
            let tail = body.rsplit(|c| ".!?".contains(c)).next().unwrap_or(&body).to_string();
            body.push(if Self::is_question(&tail) { '?' } else { '.' });
        }
        let parts = [greeting, if body.is_empty() { None } else { Some(body) }, signoff].into_iter().flatten().collect::<Vec<_>>();
        Some(parts.join("\n\n"))
    }

    pub fn fit_to_context(text: &str, context: &DictationContext, protected: &[String]) -> String {
        if context.is_secure || text.is_empty() {
            return text.to_string();
        }
        let mut s = text.to_string();
        let before = replace_all(&context.before, r"[ \t]+$", "");
        if let Some(c) = before.chars().last() {
            let first_word: String = s.chars().take_while(|ch| ch.is_alphabetic() || ch.is_numeric() || *ch == '-').collect();
            let protected_word = (first_word.chars().count() > 1 && first_word.chars().all(|ch| !ch.is_alphabetic() || ch.is_uppercase()) && first_word.chars().any(|ch| ch.is_alphabetic()))
                || protected.iter().any(|p| p.eq_ignore_ascii_case(&first_word))
                || first_word == "I"
                || context_names(context).iter().any(|n| n == &first_word)
                || looks_like_name(&first_word);
            if ".!?\n".contains(c) || before.ends_with(":\n") {
                s = cap_first(&s);
            } else if (c.is_alphabetic() || c.is_numeric() || ",;(–-".contains(c)) && !protected_word && !s.contains('\n') {
                if let Some((i, ch)) = s.char_indices().find(|(_, ch)| ch.is_alphabetic()) {
                    if ch.is_uppercase() && i == 0 {
                        let lower: String = ch.to_lowercase().collect();
                        s.replace_range(i..i + ch.len_utf8(), &lower);
                    }
                }
            }
        }
        let after: String = context.after.chars().skip_while(|c| *c == ' ' || *c == '\t').collect();
        if let Some(a) = after.chars().next() {
            if a.is_lowercase() || ",;:".contains(a) {
                while s.chars().last().map(|l| ".!?".contains(l)).unwrap_or(false) && !s.ends_with("...") {
                    s.pop();
                }
            }
        }
        s
    }

    pub fn apply_context_names(text: &str, context: &DictationContext) -> String {
        let names: Vec<String> = context_names(context).into_iter().filter(|n| n.chars().count() >= 4).collect();
        if names.is_empty() {
            return text.to_string();
        }
        replace_mapped(text, &NAME_RE, |m| {
            let w = group(m, 0).to_string();
            if names.iter().any(|n| n == &w) {
                return None;
            }
            for n in &names {
                if w.chars().count() < n.chars().count() {
                    continue;
                }
                if w.chars().next() != n.chars().next() {
                    continue;
                }
                let stem: String = w.chars().take(n.chars().count()).collect();
                let rest: String = w.chars().skip(n.chars().count()).collect();
                if stem != *n && lev(&stem, n) == 1 && rest.chars().count() <= 6 {
                    return Some(format!("{n}{rest}"));
                }
            }
            None
        })
    }
}

pub fn context_names(ctx: &DictationContext) -> Vec<String> {
    if ctx.is_secure {
        return vec![];
    }
    let text = format!("{} {}", ctx.before, ctx.after);
    let re = regex(r"(?<=[\p{L}\p{N},;]\s)(\p{Lu}[\p{Ll}]{2,})");
    let mut out = Vec::new();
    for m in re.captures_iter(&text).filter_map(|c| c.ok()) {
        let w = group(&m, 0).to_string();
        if !out.contains(&w) {
            out.push(w);
        }
    }
    out
}

fn looks_like_name(w: &str) -> bool {
    let Some(f) = w.chars().next() else { return false };
    if !f.is_uppercase() || w.chars().count() < 3 {
        return false;
    }
    let l = w.to_lowercase();
    if has(FIRST_NAMES, l.as_str()) {
        return true;
    }
    FIRST_NAMES.iter().any(|n| {
        (l.starts_with(n) && l.chars().count() - n.chars().count() <= 6)
            || (n.chars().last().map(|c| "ae".contains(c)).unwrap_or(false)
                && l.starts_with(&n.chars().take(n.chars().count().saturating_sub(1)).collect::<String>())
                && l.chars().nth(n.chars().count().saturating_sub(1)).map(|c| "áé".contains(c)).unwrap_or(false))
    })
}

fn is_number_word(w: &str) -> bool {
    let l = w.to_lowercase().trim_matches(|c: char| c.is_ascii_punctuation()).to_string();
    if l.is_empty() {
        return false;
    }
    if l.chars().next().unwrap().is_numeric() {
        return true;
    }
    if has(NUMBER_EXACT, l.as_str()) {
        return true;
    }
    NUMBER_STEMS.iter().any(|stem| l.starts_with(stem))
}

fn ordinal_index(w: &str) -> Option<usize> {
    let l = w.to_lowercase();
    ORDINALS.iter().position(|set| set.iter().any(|x| *x == l))
}

fn remove_scratched_sentences(s: &str) -> String {
    let mut parts: Vec<String> = SENTENCE_RE.find_iter(s).filter_map(|m| m.ok()).map(|m| m.as_str().to_string()).collect();
    let mut changed = false;
    let mut i = 0;
    while i < parts.len() {
        let core = trim_punct_ws(&parts[i].to_lowercase());
        if has(SCRATCH, core.as_str()) {
            parts.remove(i);
            changed = true;
            if i > 0 {
                i -= 1;
                parts.remove(i);
            }
            continue;
        }
        i += 1;
    }
    if changed { parts.concat().trim().to_string() } else { s.to_string() }
}

fn collapse_domain(s: &str) -> String {
    replace_all(s, r"(?i)\s*(?:pont|dot|\.)\s*", ".").replace(' ', "").to_lowercase()
}

pub struct AppCategoryResolver;

impl AppCategoryResolver {
    pub fn category(bundle_id: Option<&str>) -> AppCategory {
        let Some(id) = bundle_id else { return AppCategory::Other };
        BUNDLE_MAP.iter().find(|(k, _)| *k == id).map(|(_, c)| *c).unwrap_or(AppCategory::Other)
    }

    pub fn category_for(bundle_id: Option<&str>, window_title: Option<&str>) -> AppCategory {
        let base = Self::category(bundle_id);
        if base != AppCategory::Other {
            return base;
        }
        let Some(id) = bundle_id else { return base };
        if !BROWSERS.iter().any(|b| *b == id) {
            // Windows process names are also checked as browsers.
            if !WINDOWS_BROWSERS.iter().any(|b| id.eq_ignore_ascii_case(b)) {
                return base;
            }
        }
        let Some(t) = window_title.map(|s| s.to_lowercase()).filter(|s| !s.is_empty()) else { return base };
        for (cat, keys) in SITE_RULES {
            if keys.iter().any(|k| t.contains(k)) {
                return *cat;
            }
        }
        base
    }
}

const HU_WORDS: &[&str] = &[
    "az", "egy", "és", "es", "hogy", "nem", "is", "meg", "de", "ha", "már", "még", "csak", "ez", "ezt", "azt", "van", "lesz", "kell", "holnap", "ma", "szia", "kérlek", "köszönöm", "mert", "vagy", "mi", "ki", "mikor", "hol", "igen", "itt", "ott", "majd", "akkor", "nagyon", "jó", "nekem", "neked", "kedves", "aki", "ami",
];
const EN_WORDS: &[&str] = &[
    "the", "and", "to", "of", "is", "i", "you", "it", "for", "we", "that", "this", "with", "on", "are", "be", "will", "can", "please", "thanks", "let's", "my", "your", "so", "just", "what", "when", "have", "our",
];
const SCRATCH: &[&str] = &["ezt töröld", "töröld ezt", "töröld az előzőt", "az előzőt töröld", "scratch that", "delete that", "delete the last sentence"];
const NUMBER_STEMS: &[&str] = &[
    "tíz", "tizen", "húsz", "huszon", "harminc", "negyven", "ötven", "hatvan", "hetven", "nyolcvan", "kilencven", "száz", "ezer", "millió", "kettő", "három", "négy", "nyolc", "kilenc", "eleven", "twelve", "twenty", "thirty", "forty", "fifty", "hundred", "thousand", "million",
];
const NUMBER_EXACT: &[&str] = &["két", "öt", "hat", "hét", "ötkor", "hatkor", "hétkor", "egykor", "kettőkor", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
const ORDINALS: &[&[&str]] = &[
    &["első", "elsőként", "egyrészt", "first", "firstly"],
    &["második", "másodszor", "másodikként", "second", "secondly"],
    &["harmadik", "harmadszor", "harmadikként", "third", "thirdly"],
    &["negyedik", "negyedszer", "fourth"],
    &["ötödik", "ötödször", "fifth"],
    &["hatodik", "sixth"],
    &["hetedik", "seventh"],
];
const ARTICLES: &[&str] = &["a", "az", "the", "my", "our", "your", "egy", "minden", "every"];
const HU_CONJUNCTIONS: &[&str] = &[
    "hogy", "hogyha", "mert", "mivel", "ha", "de", "hanem", "aki", "akit", "akinek", "akivel", "akik", "akiket", "ami", "amit", "aminek", "amivel", "amik", "amiket", "amely", "amelyet", "amelyek", "amelynek", "amikor", "ahol", "ahova", "ahová", "ahonnan", "ahogy", "amíg", "amennyiben", "miközben", "mielőtt", "miután", "mintha",
];
const HU_NO_COMMA_BEFORE: &[&str] = &[
    "és", "s", "vagy", "még", "csak", "de", "hogy", "mert", "mint", "akár", "ha", "meg", "illetve", "sőt", "hanem", "hát", "nem", "persze", "vagyis", "azaz", "mégpedig", "kivéve", "főleg", "különösen",
];
const RELATIVE_STARTS: &[&str] = &["aki", "akit", "akinek", "akivel", "akik", "akiket", "ami", "amit", "aminek", "amivel", "amik", "amiket", "amely", "amelyet", "amelyek", "amelynek"];
const MAIN_RESUMERS: &[&str] = &["még", "már", "viszont", "azonban", "szintén", "mindig", "továbbra", "sajnos", "végül", "most"];
const HU_QUESTION_STARTS: &[&str] = &[
    "mikor", "hol", "hogyan", "miért", "mi", "mit", "mivel", "minek", "ki", "kit", "kinek", "kivel", "melyik", "hány", "hányan", "hánykor", "mennyi", "mennyiért", "milyen", "hová", "hova", "honnan", "meddig", "mióta", "merre", "miben", "kié", "tudnál", "tudnád", "tudnátok", "megtennéd", "megnéznéd", "ráérsz", "ráértek", "van", "nincs", "jössz", "jöttök",
];
const HU_POLITE_QUESTION: &[&str] = &["tudnál", "tudnád", "tudnátok", "tudnának", "megtennéd", "megnéznéd", "elküldenéd", "ráérsz", "ráértek"];
const EN_QUESTION_STARTS: &[&str] = &["what", "when", "where", "why", "how", "who", "whom", "which", "whose", "can", "could", "would", "will", "do", "does", "did", "is", "are", "should", "shall", "may", "have", "has", "am", "was", "were"];
const GREETINGS: &[&str] = &["szia", "sziasztok", "helló", "hello", "hi", "hey", "szervusz", "szevasz", "csá", "jó", "napot", "kedves"];
const ACCENTS: &[(&str, &str)] = &[
    ("koszonom", "köszönöm"), ("koszonjuk", "köszönjük"), ("koszi", "köszi"), ("szepen", "szépen"), ("valasz", "válasz"), ("valaszt", "választ"), ("valaszod", "válaszod"), ("valaszodat", "válaszodat"), ("es", "és"), ("kerlek", "kérlek"), ("kerem", "kérem"), ("udvozlettel", "üdvözlettel"), ("udv", "üdv"), ("igy", "így"), ("ugy", "úgy"), ("jo", "jó"), ("kesz", "kész"), ("keszen", "készen"), ("szamla", "számla"), ("szamlat", "számlát"), ("ajanlat", "ajánlat"), ("ajanlatot", "ajánlatot"), ("hetfo", "hétfő"), ("hetfon", "hétfőn"), ("csutortok", "csütörtök"), ("csutortokon", "csütörtökön"), ("pentek", "péntek"), ("penteken", "pénteken"), ("talalkozo", "találkozó"), ("megbeszeles", "megbeszélés"), ("holnaputan", "holnapután"), ("kosz", "kösz"), ("tobb", "több"), ("lehetoseg", "lehetőség"), ("ertekeles", "értékelés"), ("ugyfel", "ügyfél"), ("ugyfelnek", "ügyfélnek"), ("szerzodes", "szerződés"), ("szerzodest", "szerződést"), ("hatarido", "határidő"), ("kerdes", "kérdés"), ("kerdesem", "kérdésem"), ("mar", "már"),
];
const TLDS: &str = "hu|com|net|org|io|eu|ai|dev|app|co|uk|us|me|info|biz|xyz|studio|tech|shop|online";
const ACRONYMS: &str = "pr|api|ui|ux|crm|seo|sem|kpi|okr|ceo|cto|cfo|coo|hr|pdf|url|mvp|roi|b2b|b2c|saas|qa|sql|ai|sms|faq|erp|vpn|cv|gdpr|ppc|cta|sdk|llm|Pr|Api|Ui|Ux|Crm|Seo|Kpi|Ceo|Cto|Hr|Pdf|Url|Mvp|Roi|Saas|Sql|Ai|Gdpr|Llm";
const STRONG_SIGNOFFS: &str = "baráti üdvözlettel|üdvözlettel|tisztelettel|köszönettel|üdv|best regards|kind regards|regards|best wishes";
const WEAK_SIGNOFFS: &str = "köszönöm|köszi|thanks|thank you|best|cheers|szép napot";
const COMMON_NON_NAMES: &[&str] = &["köszönöm", "köszi", "holnap", "ma", "minden", "mindenkinek", "küldöm", "csak", "még", "már", "itt", "most", "thanks", "thank", "please", "just", "the", "a", "az", "egy", "és", "is", "nem", "jó"];
const FIRST_NAMES: &[&str] = &[
    "anna", "péter", "attila", "dorottya", "gábor", "lászló", "istván", "zoltán", "józsef", "jános", "ferenc", "sándor", "tamás", "zsolt", "tibor", "andrás", "csaba", "imre", "balázs", "gergely", "dávid", "dániel", "ádám", "bence", "máté", "levente", "márk", "krisztián", "norbert", "róbert", "richárd", "viktor", "szabolcs", "ákos", "gyula", "miklós", "károly", "béla", "mihály", "mária", "erzsébet", "katalin", "éva", "ilona", "zsuzsanna", "judit", "ágnes", "andrea", "erika", "krisztina", "eszter", "edit", "anita", "mónika", "tímea", "nóra", "réka", "zsófia", "viktória", "dóra", "petra", "kitti", "lilla", "fanni", "boglárka", "orsolya", "beáta", "szilvia", "melinda", "barbara", "noémi", "vivien", "rita", "ildikó", "gabriella", "csilla", "emese", "eniko", "enikő", "john", "michael", "david", "james", "robert", "mary", "sarah", "emma", "peter", "paul", "mark", "anne", "laura", "tom", "alex",
];
const BROWSERS: &[&str] = &[
    "com.google.Chrome", "com.google.Chrome.canary", "com.apple.Safari", "com.apple.SafariTechnologyPreview", "company.thebrowser.Browser", "company.thebrowser.dia", "org.mozilla.firefox", "com.microsoft.edgemac", "com.brave.Browser", "com.operasoftware.Opera", "com.vivaldi.Vivaldi", "ai.perplexity.comet", "app.zen-browser.zen",
];
const WINDOWS_BROWSERS: &[&str] = &["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"];
const SITE_RULES: &[(AppCategory, &[&str])] = &[
    (AppCategory::Email, &["gmail", "outlook", "proton mail", "yahoo mail", "superhuman", "fastmail", "icloud mail", "freemail", "citromail"]),
    (AppCategory::Work, &["slack", "microsoft teams", "discord", "notion", "linear", "jira", "clickup", "github", "asana", "trello", "google docs", "google dokumentumok", "figma", "confluence", "monday.com", "airtable"]),
    (AppCategory::Personal, &["whatsapp", "messenger", "telegram", "instagram", "facebook", "signal"]),
];
const BUNDLE_MAP: &[(&str, AppCategory)] = &[
    ("com.apple.MobileSMS", AppCategory::Personal),
    ("net.whatsapp.WhatsApp", AppCategory::Personal),
    ("desktop.WhatsApp", AppCategory::Personal),
    ("WhatsApp.exe", AppCategory::Personal),
    ("ru.keepcoder.Telegram", AppCategory::Personal),
    ("org.telegram.desktop", AppCategory::Personal),
    ("Telegram.exe", AppCategory::Personal),
    ("com.facebook.archon", AppCategory::Personal),
    ("com.facebook.archon.developerID", AppCategory::Personal),
    ("org.whispersystems.signal-desktop", AppCategory::Personal),
    ("com.viber.osx", AppCategory::Personal),
    ("com.tinyspeck.slackmacgap", AppCategory::Work),
    ("slack.exe", AppCategory::Work),
    ("com.microsoft.teams", AppCategory::Work),
    ("com.microsoft.teams2", AppCategory::Work),
    ("ms-teams.exe", AppCategory::Work),
    ("com.hnc.Discord", AppCategory::Work),
    ("Discord.exe", AppCategory::Work),
    ("com.linear", AppCategory::Work),
    ("notion.id", AppCategory::Work),
    ("Notion.exe", AppCategory::Work),
    ("com.figma.Desktop", AppCategory::Work),
    ("com.clickup.desktop-app", AppCategory::Work),
    ("com.apple.mail", AppCategory::Email),
    ("com.microsoft.Outlook", AppCategory::Email),
    ("OUTLOOK.EXE", AppCategory::Email),
    ("olk.exe", AppCategory::Email),
    ("com.readdle.smartemail-Mac", AppCategory::Email),
    ("com.superhuman.electron", AppCategory::Email),
];

static BACKTRACK_RE: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)\s*,\s*(?:nem\s*,?\s*inkább|nem\s*,?\s*hanem|bocsánat|bocsi|bocs|pardon|javítok|helyesbítek|akarom mondani|actually\s*,?\s*no|no\s*,?\s*wait|sorry\s*,?\s*i mean|or rather)\s*[,:]?\s+"));
static SENTENCE_BOUNDARY_RE: LazyLock<Regex> = LazyLock::new(|| regex(r"[.!?\n]\s*"));
static SENTENCE_RE: LazyLock<Regex> = LazyLock::new(|| regex(r"[^.!?\n]+[.!?]*\s*|\n+\s*|[.!?]+\s*"));
static ORDINAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    let mut all: Vec<&str> = ORDINALS.iter().copied().flatten().copied().collect();
    all.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    regex(&format!(r"(?i)(?<![\p{{L}}\p{{N}}])({})(?![\p{{L}}\p{{N}}])[,:]?", all.join("|")))
});
static HU_CONJ_RE: LazyLock<Regex> = LazyLock::new(|| regex(&format!(r"(?<=[\p{{L}}\p{{N}}])(\s+)({})(?![\p{{L}}\p{{N}}])", HU_CONJUNCTIONS.join("|"))));
static WORD_RE: LazyLock<Regex> = LazyLock::new(|| regex(r"(?<![\p{L}\p{N}])[\p{L}]+(?![\p{L}\p{N}])"));
static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| regex(&format!(r"(?i)(?<![\p{{L}}\p{{N}}@.])([\p{{L}}\p{{N}}._-]+)\s+(?:kukac|at)\s+((?:[\p{{L}}\p{{N}}-]+\s*(?:pont|dot|\.)\s*)+(?:{TLDS}))(?![\p{{L}}\p{{N}}])")));
static DOMAIN_RE: LazyLock<Regex> = LazyLock::new(|| regex(&format!(r"(?i)(?<![\p{{L}}\p{{N}}@.])((?:[\p{{L}}\p{{N}}-]+\s+(?:pont|dot)\s+)+(?:{TLDS}))(?![\p{{L}}\p{{N}}])((?:\s+(?:perjel|per|slash)\s+[\p{{L}}\p{{N}}-]+)*)")));
static ACRONYM_RE: LazyLock<Regex> = LazyLock::new(|| regex(&format!(r"(?<![\p{{L}}\p{{N}}])({ACRONYMS})(?=-|(?![\p{{L}}\p{{N}}]))")));
static GREETING_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| regex(r"^((?i:szia|sziasztok|helló|hello|hi|hey|szervusz|szevasz))\s+(?=\p{Ll})"));
static MAIL_GREETING_RE: LazyLock<Regex> = LazyLock::new(|| regex(r"^((?i:kedves|tisztelt|szia|sziasztok|szervusz|helló|hello|jó napot|dear|hi))\s+([\p{L}.-]+)(\s+\p{Lu}[\p{L}.-]+)?\s*[,!:.]?\s+"));
static STRONG_SIGNOFF_RE: LazyLock<Regex> = LazyLock::new(|| regex(&format!(r"[,.;]?\s+((?i:{STRONG_SIGNOFFS}))[,:.!]?\s+([\p{{L}}.-]+(?:\s+\p{{Lu}}[\p{{L}}.-]*)?)[.!]?\s*$")));
static WEAK_SIGNOFF_RE: LazyLock<Regex> = LazyLock::new(|| regex(&format!(r"[,.;]?\s+((?i:{WEAK_SIGNOFFS}))[,:.!]?\s+(\p{{Lu}}[\p{{L}}.-]*(?:\s+\p{{Lu}}[\p{{L}}.-]*)?)[.!]?\s*$")));
static NAME_RE: LazyLock<Regex> = LazyLock::new(|| regex(r"(?<![\p{L}\p{N}])\p{Lu}[\p{Ll}]{3,}"));

// Silence unused import if fold is only used by pipeline.
#[allow(dead_code)]
fn _fold(s: &str) -> String { fold_diacritics(s) }
