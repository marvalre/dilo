//! Personal dictionary applied to every transcript before pasting.
//!
//! - correction: the model writes `from`, we write `to` ("cloud" → "Claude").
//! - shortcut: you say `from`, `to` is pasted ("mi correo" → "yo@mail.com").
//!
//! Matching is case-insensitive and respects word boundaries. Parakeet cannot be
//! taught new words, so correcting after transcription is how the dictionary works.

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    Correction,
    Shortcut,
}

impl RuleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleKind::Correction => "correction",
            RuleKind::Shortcut => "shortcut",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "correction" => Some(RuleKind::Correction),
            "shortcut" => Some(RuleKind::Shortcut),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub id: i64,
    pub kind: RuleKind,
    pub from: String,
    pub to: String,
    pub enabled: bool,
}

/// Lowercase, strip punctuation and collapse spaces — for "the whole utterance is the trigger".
fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn pattern(from: &str) -> Option<Regex> {
    let from = from.trim();
    if from.is_empty() {
        return None;
    }
    let starts_word = from.chars().next().is_some_and(char::is_alphanumeric);
    let ends_word = from.chars().last().is_some_and(char::is_alphanumeric);
    // Let any run of whitespace in the trigger match any whitespace in the text.
    let body = from.split_whitespace().map(regex::escape).collect::<Vec<_>>().join(r"\s+");
    let pat = format!(
        "{}{}{}",
        if starts_word { r"\b" } else { "" },
        body,
        if ends_word { r"\b" } else { "" }
    );
    RegexBuilder::new(&pat).case_insensitive(true).build().ok()
}

pub fn apply(text: &str, rules: &[Rule]) -> String {
    let active: Vec<&Rule> = rules.iter().filter(|r| r.enabled && !r.from.trim().is_empty()).collect();

    // A shortcut spoken on its own ("Mi correo.") pastes exactly its expansion.
    let whole = normalize(text);
    if let Some(r) = active
        .iter()
        .find(|r| r.kind == RuleKind::Shortcut && normalize(&r.from) == whole)
    {
        return r.to.clone();
    }

    // Longer triggers first so "mi correo del trabajo" wins over "mi correo".
    let mut ordered = active;
    ordered.sort_by_key(|r| std::cmp::Reverse(r.from.len()));
    let mut out = text.to_string();
    for r in ordered {
        if let Some(re) = pattern(&r.from) {
            out = re.replace_all(&out, regex::NoExpand(&r.to)).into_owned();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: RuleKind, from: &str, to: &str) -> Rule {
        Rule { id: 0, kind, from: from.into(), to: to.into(), enabled: true }
    }

    #[test]
    fn corrections_are_case_insensitive_and_whole_word() {
        let rules = vec![rule(RuleKind::Correction, "cloud", "Claude")];
        assert_eq!(apply("Le pregunté a Cloud y a cloud.", &rules), "Le pregunté a Claude y a Claude.");
        assert_eq!(apply("cloudflare no cambia", &rules), "cloudflare no cambia");
    }

    #[test]
    fn multi_word_triggers_match_any_spacing() {
        let rules = vec![rule(RuleKind::Correction, "m visuals", "M Visuals")];
        assert_eq!(apply("Trabajo en m  visuals hoy", &rules), "Trabajo en M Visuals hoy");
    }

    #[test]
    fn shortcut_alone_pastes_exact_expansion() {
        let rules = vec![rule(RuleKind::Shortcut, "mi correo", "yo@mail.com")];
        assert_eq!(apply("Mi correo.", &rules), "yo@mail.com");
        assert_eq!(apply("Mándalo a mi correo por favor", &rules), "Mándalo a yo@mail.com por favor");
    }

    #[test]
    fn longest_trigger_wins_and_disabled_rules_are_ignored() {
        let mut off = rule(RuleKind::Correction, "hola", "ADIÓS");
        off.enabled = false;
        let rules = vec![
            rule(RuleKind::Shortcut, "mi correo", "a@x.com"),
            rule(RuleKind::Shortcut, "mi correo del trabajo", "b@work.com"),
            off,
        ];
        assert_eq!(apply("hola, mi correo del trabajo es este", &rules), "hola, b@work.com es este");
    }

    #[test]
    fn replacement_text_is_literal() {
        let rules = vec![rule(RuleKind::Correction, "precio", "$100")];
        assert_eq!(apply("el precio", &rules), "el $100");
    }
}
