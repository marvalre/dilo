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

/// Longest trigger we accept; keeps the combined regex small and predictable.
pub const MAX_TRIGGER_CHARS: usize = 200;
/// Largest replacement we accept (a pasted snippet, not a document).
pub const MAX_REPLACEMENT_CHARS: usize = 10_000;

/// Checks a rule coming from the UI. Returns a user-facing message on error.
pub fn validate(from: &str, to: &str) -> Result<(), String> {
    if from.trim().is_empty() {
        return Err("Escribe la palabra o frase".into());
    }
    if from.trim().chars().count() > MAX_TRIGGER_CHARS {
        return Err(format!("La palabra o frase es demasiado larga (máximo {MAX_TRIGGER_CHARS} caracteres)"));
    }
    if to.chars().count() > MAX_REPLACEMENT_CHARS {
        return Err(format!("El texto de reemplazo es demasiado largo (máximo {MAX_REPLACEMENT_CHARS} caracteres)"));
    }
    Ok(())
}

/// Characters of scripts written without spaces: a word boundary makes no sense next to them.
fn is_word(c: char) -> bool {
    (c.is_alphanumeric() || c == '_') && !crate::stats::is_unspaced_script(c)
}

/// Applies every enabled rule in a single pass, so replaced text is never
/// rewritten again by another rule. Longest trigger wins where they overlap.
pub fn apply(text: &str, rules: &[Rule]) -> String {
    let mut active: Vec<&Rule> = rules.iter().filter(|r| r.enabled && !r.from.trim().is_empty()).collect();

    if let Some(r) = active.iter().find(|r| r.kind == RuleKind::Shortcut && is_whole_utterance(text, &r.from)) {
        return r.to.clone();
    }

    // Stable sort: equal-length triggers keep their order.
    active.sort_by_key(|r| std::cmp::Reverse(r.from.trim().chars().count()));

    // One anchored regex per rule (tried in priority order at each candidate position, so a longer
    // trigger that fails the word-boundary check falls back to a shorter one), plus one alternation
    // used only to jump to the next candidate.
    let build = |pat: &str| RegexBuilder::new(pat).case_insensitive(true).size_limit(50 << 20).build().ok();
    let compiled: Vec<(&Rule, regex::Regex)> =
        active.iter().filter_map(|r| build(&format!("^(?:{})", body(&r.from))).map(|re| (*r, re))).collect();
    if compiled.is_empty() {
        return text.to_string();
    }
    let alternation = compiled.iter().map(|(r, _)| format!("(?:{})", body(&r.from))).collect::<Vec<_>>().join("|");
    let Some(scan) = build(&alternation) else {
        return text.to_string();
    };

    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut pos = 0;
    while pos <= text.len() {
        let Some(m) = scan.find_at(text, pos) else { break };
        let start = m.start();
        let before = text[..start].chars().next_back();
        let tail = &text[start..];
        let mut hit: Option<(&Rule, usize)> = None;
        for (rule, re) in &compiled {
            let Some(rm) = re.find(tail) else { continue };
            if rm.is_empty() {
                continue;
            }
            let end = start + rm.end();
            let matched = &text[start..end];
            let after = text[end..].chars().next();
            let lastc = matched.chars().next_back();
            // Whole words only: no word character glued to either edge of the match.
            // A trigger starting with a symbol ("@x") also must not hang off a word ("a@x").
            let ok_start = !before.is_some_and(is_word);
            let ok_end = !(lastc.is_some_and(is_word) && after.is_some_and(is_word));
            if ok_start && ok_end {
                hit = Some((rule, end));
                break;
            }
        }
        let Some((rule, end)) = hit else {
            pos = start + text[start..].chars().next().map_or(1, char::len_utf8);
            continue;
        };
        out.push_str(&text[last..start]);
        let replacement = if rule.kind == RuleKind::Correction { match_case(&text[start..end], &rule.to) } else { rule.to.clone() };
        out.push_str(&replacement);
        last = end;
        pos = end;
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

    #[test]
    fn longer_trigger_failing_boundary_falls_back_to_shorter() {
        let rules = vec![rule(RuleKind::Correction, "code review", "CR"), rule(RuleKind::Correction, "code", "código")];
        assert_eq!(apply("code reviewed ayer", &rules), "código reviewed ayer");
        assert_eq!(apply("code review hoy", &rules), "CR hoy");
    }

    #[test]
    fn regex_metacharacters_in_triggers_are_literal() {
        let rules = vec![
            rule(RuleKind::Correction, "a.b", "X"),
            rule(RuleKind::Correction, "(foo)", "Y"),
            rule(RuleKind::Correction, "[x]*", "Z"),
            rule(RuleKind::Correction, r"\d+", "N"),
            rule(RuleKind::Correction, "a|b", "P"),
        ];
        assert_eq!(apply("a.b axb", &rules), "X axb");
        assert_eq!(apply("un (foo) y foo", &rules), "un Y y foo");
        assert_eq!(apply("[x]* x", &rules), "Z x");
        assert_eq!(apply(r"\d+ 12", &rules), "N 12");
        assert_eq!(apply("a|b a b", &rules), "P a b");
    }

    #[test]
    fn empty_inputs_and_empty_rules_are_safe() {
        assert_eq!(apply("", &[rule(RuleKind::Correction, "a", "b")]), "");
        assert_eq!(apply("hola", &[]), "hola");
        assert_eq!(apply("hola", &[rule(RuleKind::Correction, "   ", "x")]), "hola");
        assert_eq!(apply("hola mundo", &[rule(RuleKind::Correction, "hola", "")]), " mundo");
    }

    #[test]
    fn unicode_case_and_cjk() {
        let rules = vec![rule(RuleKind::Correction, "ñandú", "ñandu"), rule(RuleKind::Correction, "你好", "hola")];
        assert_eq!(apply("El ÑANDÚ corre", &rules), "El Ñandu corre");
        assert_eq!(apply("他说你好吗", &rules), "他说hola吗");
        assert_eq!(apply("🙂 ñandú 🙂", &rules), "🙂 ñandu 🙂");
    }

    #[test]
    fn same_trigger_twice_and_adjacent_matches() {
        let rules = vec![rule(RuleKind::Correction, "ok", "OK"), rule(RuleKind::Correction, "ok", "Vale")];
        assert_eq!(apply("ok ok. ok", &rules), "OK OK. OK");
    }

    #[test]
    fn huge_input_with_many_rules_finishes() {
        let rules: Vec<Rule> = (0..200).map(|i| rule(RuleKind::Correction, &format!("palabra{i}"), "X")).collect();
        let text = "palabra7 relleno ".repeat(20_000);
        let out = apply(&text, &rules);
        assert!(out.starts_with("X relleno X relleno"));
        // Many boundary failures ("palabra7s") must not go quadratic.
        let glued = "palabra7s ".repeat(20_000);
        assert_eq!(apply(&glued, &rules), glued);
    }

    #[test]
    fn validate_limits() {
        assert!(validate("  ", "x").is_err());
        assert!(validate(&"a".repeat(MAX_TRIGGER_CHARS + 1), "x").is_err());
        assert!(validate("a", &"b".repeat(MAX_REPLACEMENT_CHARS + 1)).is_err());
        assert!(validate("a", "").is_ok());
    }
}
