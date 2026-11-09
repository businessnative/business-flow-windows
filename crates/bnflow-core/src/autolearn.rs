use crate::models::DictionaryEntry;
use crate::text_rules::TextRules;
use crate::util::{is_punct, lev};

pub struct AutoLearn;

impl AutoLearn {
    pub fn correction(inserted: &str, pasted: &str, later: &str, dictionary: &[DictionaryEntry]) -> Option<(String, String)> {
        if pasted == later {
            return None;
        }
        let a = tokens(pasted);
        let b = tokens(later);
        if a.len() != b.len() || a.is_empty() {
            return None;
        }
        let mut p = 0;
        while p < a.len() && a[p] == b[p] {
            p += 1;
        }
        let mut sfx = 0;
        while sfx < a.len() - p && a[a.len() - 1 - sfx] == b[b.len() - 1 - sfx] {
            sfx += 1;
        }
        if a.len() - p - sfx != 1 {
            return None;
        }
        let old = core(&a[p]);
        let new = core(&b[p]);
        if old == new || old.chars().count() < 3 || new.chars().count() < 3 {
            return None;
        }
        if !new.chars().all(|c| c.is_alphabetic() || c.is_numeric() || c == '-' || c == '.') {
            return None;
        }
        let words = TextRules::language(inserted);
        let _ = words;
        if !crate::util::words(inserted).iter().any(|w| w == &old.to_lowercase()) {
            return None;
        }
        if similarity(&old.to_lowercase(), &new.to_lowercase()) < 0.5 {
            return None;
        }
        let known = dictionary.iter().any(|e| e.term.eq_ignore_ascii_case(&new) && e.heard_as.iter().any(|h| h.eq_ignore_ascii_case(&old)));
        if known {
            return None;
        }
        if old.to_lowercase() == new.to_lowercase() && p == 0 {
            return None;
        }
        Some((old, new))
    }

    pub fn learn(heard: &str, term: &str, dict: &mut Vec<DictionaryEntry>) {
        if let Some(i) = dict.iter().position(|e| e.term == term) {
            if !dict[i].heard_as.iter().any(|h| h == heard) {
                dict[i].heard_as.push(heard.to_string());
            }
        } else {
            let heard_as = if heard.eq_ignore_ascii_case(term) { vec![] } else { vec![heard.to_string()] };
            dict.push(DictionaryEntry { id: uuid::Uuid::new_v4(), term: term.to_string(), heard_as, auto_learned: true });
        }
    }
}

fn tokens(s: &str) -> Vec<String> {
    s.split_whitespace().map(|w| w.to_string()).collect()
}

fn core(w: &str) -> String {
    w.trim_matches(|c: char| c.is_whitespace() || is_punct(c) || c.is_ascii_punctuation()).to_string()
}

fn similarity(a: &str, b: &str) -> f64 {
    let x: Vec<char> = a.chars().collect();
    let y: Vec<char> = b.chars().collect();
    let m = x.len().max(y.len());
    if m == 0 {
        1.0
    } else {
        1.0 - (lev(&x.iter().collect::<String>(), &y.iter().collect::<String>()) as f64) / m as f64
    }
}
