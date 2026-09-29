//! Personal dictionary applied to every transcript before pasting.
//!
//! - correction: the model writes `from`, we write `to` ("cloud" → "Claude").
//! - shortcut: you say `from`, `to` is pasted ("mi correo" → "yo@mail.com").
//!
//! Matching is case-insensitive and respects word boundaries. Parakeet cannot be
//! taught new words, so correcting after transcription is how the dictionary works.

use regex::RegexBuilder;
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

/// A trigger spoken alone: words must match ignoring punctuation, but a trigger
/// made of symbols ("c++", "🙂") must match exactly (ignoring case and edges).
fn is_whole_utterance(text: &str, trigger: &str) -> bool {
    let (nt, nr) = (normalize(text), normalize(trigger));
    if nr.is_empty() {
        return text.trim().to_lowercase() == trigger.trim().to_lowercase();
    }
    let symbolic = trigger.trim().chars().any(|c| !c.is_alphanumeric() && !c.is_whitespace());
    if symbolic {
        let strip = |s: &str| s.trim().trim_end_matches(['.', ',', '!', '?', '¡', '¿']).trim().to_lowercase();
        return strip(text) == strip(trigger);
    }
    nt == nr
}

fn body(from: &str) -> String {
    from.split_whitespace().map(regex::escape).collect::<Vec<_>>().join(r"\s+")
}

/// Keeps a leading capital: "Okey, vamos" with okey→ok gives "Ok, vamos".
fn match_case(matched: &str, to: &str) -> String {
    let starts_upper = matched.chars().next().is_some_and(char::is_uppercase);
    let mut chars = to.chars();
    match chars.next() {
        Some(c) if starts_upper && c.is_lowercase() => c.to_uppercase().chain(chars).collect(),
        _ => to.to_string(),
    }
}

/// Applies every enabled rule in a single pass, so replaced text is never
/// rewritten again by another rule. Longest trigger wins where they overlap.
pub fn apply(text: &str, rules: &[Rule]) -> String {
    let mut active: Vec<&Rule> = rules.iter().filter(|r| r.enabled && !r.from.trim().is_empty()).collect();

    if let Some(r) = active.iter().find(|r| r.kind == RuleKind::Shortcut && is_whole_utterance(text, &r.from)) {
        return r.to.clone();
    }

    active.sort_by_key(|r| std::cmp::Reverse(r.from.trim().chars().count()));
    if active.is_empty() {
        return text.to_string();
    }
    let alternation = active.iter().map(|r| format!("({})", body(&r.from))).collect::<Vec<_>>().join("|");
    let Ok(re) = RegexBuilder::new(&alternation).case_insensitive(true).build() else {
        return text.to_string();
    };

    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut pos = 0;
    while pos <= text.len() {
        let Some(caps) = re.captures_at(text, pos) else { break };
        let m = caps.get(0).unwrap();
        let idx = (1..caps.len()).find(|&i| caps.get(i).is_some()).unwrap_or(1) - 1;
        let rule = active[idx];
        let before = text[..m.start()].chars().next_back();
        let after = text[m.end()..].chars().next();
        let first = m.as_str().chars().next();
        let lastc = m.as_str().chars().next_back();
        // Whole words only: no word character glued to either edge of the match.
        let ok_start = !(first.is_some_and(is_word) && before.is_some_and(is_word))
            && !(first.is_some_and(|c| !is_word(c)) && before.is_some_and(is_word));
        let ok_end = !(lastc.is_some_and(is_word) && after.is_some_and(is_word));
        if m.is_empty() || !ok_start || !ok_end {
            pos = m.start() + text[m.start()..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        out.push_str(&text[last..m.start()]);
        let replacement = if rule.kind == RuleKind::Correction { match_case(m.as_str(), &rule.to) } else { rule.to.clone() };
        out.push_str(&replacement);
        last = m.end();
        pos = m.end();
    }
    out.push_str(&text[last..]);
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
    fn rules_never_rewrite_each_other() {
        let rules = vec![
            rule(RuleKind::Correction, "cloud code", "Claude Code"),
            rule(RuleKind::Correction, "code", "código"),
            rule(RuleKind::Shortcut, "mi correo", "ejemplo@gmail.com"),
            rule(RuleKind::Correction, "gmail", "Gmail"),
        ];
        assert_eq!(apply("uso cloud code a diario", &rules), "uso Claude Code a diario");
        assert_eq!(apply("escribe a mi correo ya", &rules), "escribe a ejemplo@gmail.com ya");
        assert_eq!(apply("el code y gmail", &rules), "el código y Gmail");
    }

    #[test]
    fn symbolic_and_empty_triggers_are_strict() {
        let rules = vec![rule(RuleKind::Shortcut, "c++", "C plus plus"), rule(RuleKind::Shortcut, "🙂", "sonrisa")];
        assert_eq!(apply("C.", &rules), "C.");
        assert_eq!(apply("...", &rules), "...");
        assert_eq!(apply("c++", &rules), "C plus plus");
    }

    #[test]
    fn keeps_leading_capital_and_punctuation_boundaries() {
        let rules = vec![rule(RuleKind::Correction, "okey", "ok"), rule(RuleKind::Correction, "@marcelo", "@mv")];
        assert_eq!(apply("Okey, vamos. okey", &rules), "Ok, vamos. ok");
        assert_eq!(apply("yo@marcelo.com y @marcelo", &rules), "yo@marcelo.com y @mv");
    }

    #[test]
    fn accented_words_respect_boundaries() {
        let rules = vec![rule(RuleKind::Correction, "está", "ESTÁ"), rule(RuleKind::Correction, "niño", "chico")];
        assert_eq!(apply("Está el niño y los niños estás", &rules), "ESTÁ el chico y los niños estás");
    }

    #[test]
    fn replacement_text_is_literal() {
        let rules = vec![rule(RuleKind::Correction, "precio", "$100")];
        assert_eq!(apply("el precio", &rules), "el $100");
    }
}
