use crate::models::{DictionaryEntry, Snippet};
use uuid::Uuid;

pub struct DictionaryEditing;

impl DictionaryEditing {
    pub fn parse_heard(s: &str) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for part in s.split(',') {
            let t = part.trim();
            if t.is_empty() {
                continue;
            }
            if seen.insert(t.to_lowercase()) {
                out.push(t.to_string());
            }
        }
        out
    }

    pub fn make(term: &str, heard: &str) -> Option<DictionaryEntry> {
        let t = term.trim();
        if t.is_empty() {
            return None;
        }
        let heard_as = Self::parse_heard(heard).into_iter().filter(|h| !h.eq_ignore_ascii_case(t)).collect();
        Some(DictionaryEntry { id: Uuid::new_v4(), term: t.to_string(), heard_as, auto_learned: false })
    }

    pub fn insert(entry: DictionaryEntry, list: &[DictionaryEntry]) -> Vec<DictionaryEntry> {
        let mut out = list.to_vec();
        if let Some(i) = out.iter().position(|e| e.term.eq_ignore_ascii_case(&entry.term)) {
            let mut m = out.remove(i);
            m.term = entry.term;
            for h in entry.heard_as {
                if !m.heard_as.iter().any(|x| x.eq_ignore_ascii_case(&h)) {
                    m.heard_as.push(h);
                }
            }
            m.auto_learned = m.auto_learned && entry.auto_learned;
            out.insert(0, m);
            return out;
        }
        out.insert(0, entry);
        out
    }
}

pub struct SnippetEditing;

impl SnippetEditing {
    pub fn save(id: Option<Uuid>, trigger: &str, expansion: &str, list: &[Snippet]) -> Vec<Snippet> {
        let t = trigger.trim();
        if t.is_empty() || expansion.is_empty() {
            return list.to_vec();
        }
        let mut out = list.to_vec();
        if let Some(id) = id {
            if let Some(i) = out.iter().position(|s| s.id == id) {
                out[i].trigger = t.to_string();
                out[i].expansion = expansion.to_string();
                out.retain(|s| s.id == id || !s.trigger.eq_ignore_ascii_case(t));
                return out;
            }
        }
        out.retain(|s| !s.trigger.eq_ignore_ascii_case(t));
        out.insert(0, Snippet::new(t, expansion));
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Same(String),
    Added(String),
    Removed(String),
}

pub struct WordDiff;

impl WordDiff {
    pub fn diff(a: &str, b: &str) -> Vec<DiffOp> {
        let x: Vec<&str> = a.split_whitespace().collect();
        let y: Vec<&str> = b.split_whitespace().collect();
        let n = x.len();
        let m = y.len();
        if n.saturating_mul(m) > 4_000_000 {
            return vec![DiffOp::Removed(a.to_string()), DiffOp::Added(b.to_string())];
        }
        let mut t = vec![vec![0i32; m + 1]; n + 1];
        if n > 0 && m > 0 {
            for i in (0..n).rev() {
                for j in (0..m).rev() {
                    t[i][j] = if x[i] == y[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
                }
            }
        }
        let mut ops = Vec::new();
        let mut i = 0;
        let mut j = 0;
        while i < n && j < m {
            if x[i] == y[j] {
                ops.push(DiffOp::Same(x[i].to_string()));
                i += 1;
                j += 1;
            } else if t[i + 1][j] >= t[i][j + 1] {
                ops.push(DiffOp::Removed(x[i].to_string()));
                i += 1;
            } else {
                ops.push(DiffOp::Added(y[j].to_string()));
                j += 1;
            }
        }
        while i < n {
            ops.push(DiffOp::Removed(x[i].to_string()));
            i += 1;
        }
        while j < m {
            ops.push(DiffOp::Added(y[j].to_string()));
            j += 1;
        }
        ops
    }
}

pub const POLISH_PRESETS: &[(&str, &str, &str)] = &[
    ("clean", "Tisztázás", "Javítsd a nyelvtant, a helyesírást és az írásjeleket, simítsd a fogalmazást, de a jelentést, a hangnemet és a nyelvet tartsd meg."),
    ("concise", "Tömörítés", "Fogalmazd meg rövidebben és lényegretörőbben, minden fontos információ maradjon meg."),
    ("formal", "Hivatalos", "Írd át udvarias, professzionális, üzleti hangnemre."),
    ("friendly", "Barátságos", "Írd át barátságos, közvetlen, de igényes hangnemre."),
    ("bullets", "Pontokba szedés", "Szedd tömör, áttekinthető felsorolásba (gondolatjeles pontok)."),
    ("to-en", "Angolra fordítás", "Fordítsd természetes, igényes angolra."),
    ("to-hu", "Magyarra fordítás", "Fordítsd természetes, igényes magyarra."),
];
