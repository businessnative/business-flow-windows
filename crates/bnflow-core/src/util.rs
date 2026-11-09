use fancy_regex::{Captures, Regex};

pub fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|err| panic!("invalid regex `{pattern}`: {err}"))
}

pub fn group<'a>(caps: &'a Captures<'a>, index: usize) -> &'a str {
    caps.get(index).map(|m| m.as_str()).unwrap_or("")
}

/// Replace non-overlapping matches from the end. `None` keeps the original match.
pub fn replace_mapped(input: &str, re: &Regex, mut f: impl FnMut(&Captures) -> Option<String>) -> String {
    let mut spans = Vec::new();
    for caps in re.captures_iter(input) {
        let Ok(caps) = caps else { continue };
        let Some(all) = caps.get(0) else { continue };
        if let Some(rep) = f(&caps) {
            spans.push((all.start(), all.end(), rep));
        }
    }
    if spans.is_empty() {
        return input.to_string();
    }
    let mut out = input.to_string();
    for (start, end, rep) in spans.into_iter().rev() {
        out.replace_range(start..end, &rep);
    }
    out
}

pub fn replace_all(input: &str, pattern: &str, replacement: &str) -> String {
    let re = regex(pattern);
    re.replace_all(input, replacement).into_owned()
}

/// Literal replacement (no `$` expansion).
pub fn replace_literal(input: &str, pattern: &str, replacement: &str) -> String {
    let re = regex(pattern);
    replace_mapped(input, &re, |_| Some(replacement.to_string()))
}

pub fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !(c.is_alphabetic() || c.is_numeric() || c == '\'' || c == '-'))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

pub fn cap_first(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let Some(i) = chars.iter().position(|c| c.is_alphabetic()) else {
        return s.to_string();
    };
    let upper: String = chars[i].to_uppercase().collect();
    let mut out = String::new();
    for (j, ch) in chars.iter().enumerate() {
        if j == i {
            out.push_str(&upper);
        } else {
            out.push(*ch);
        }
    }
    out
}

pub fn is_capitalized(w: &str) -> bool {
    w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}

pub fn lev(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

pub fn fold_diacritics(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd()
        .filter(|c| !matches!(*c, '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}'))
        .collect::<String>()
        .to_lowercase()
}

pub fn trim_punct_ws(s: &str) -> String {
    s.trim_matches(|c: char| c.is_whitespace() || is_punct(c)).to_string()
}

pub fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '„' | '”' | '“' | '’' | '‘' | '«' | '»' | '–' | '—' | '…' | '·' | '‚' | '‹' | '›'
        )
}

pub fn word_count(s: &str) -> usize {
    s.split(|c: char| c.is_whitespace())
        .filter(|tok| tok.chars().any(|c| c.is_alphabetic() || c.is_numeric()))
        .count()
}
