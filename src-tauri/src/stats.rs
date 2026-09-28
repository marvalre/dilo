//! Pure statistics over dictation history. No I/O here so it is easy to test.

use chrono::{DateTime, Duration, NaiveDate, TimeZone};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Typing speed used to estimate "time saved" versus typing the same words.
const TYPING_WPM: f64 = 40.0;

/// A word is a whitespace-separated token with at least one letter or digit.
pub fn count_words(text: &str) -> u32 {
    text.split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .count() as u32
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub created_at_ms: i64,
    pub duration_ms: i64,
    pub words: u32,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Stats {
    pub total_words: u64,
    pub words_today: u64,
    pub words_week: u64,
    pub total_dictations: u64,
    pub avg_words_per_day: f64,
    pub avg_words_per_dictation: f64,
    pub avg_wpm: f64,
    pub current_streak: u32,
    pub best_streak: u32,
    pub time_saved_min: f64,
    /// Oldest first, exactly 30 entries ending today: ("YYYY-MM-DD", words).
    pub last_30_days: Vec<(String, u64)>,
    /// Largest first: (ISO language code or "?", words).
    pub languages: Vec<(String, u64)>,
}

fn local_day<Tz: TimeZone>(ms: i64, tz: &Tz) -> NaiveDate {
    DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .with_timezone(tz)
        .date_naive()
}

pub fn compute<Tz: TimeZone>(entries: &[Entry], now_ms: i64, tz: &Tz) -> Stats {
    let today = local_day(now_ms, tz);
    let mut per_day: BTreeMap<NaiveDate, u64> = BTreeMap::new();
    let mut langs: HashMap<String, u64> = HashMap::new();
    let mut total_words = 0u64;
    let mut total_ms = 0i64;

    for e in entries {
        let w = e.words as u64;
        total_words += w;
        total_ms += e.duration_ms.max(0);
        *per_day.entry(local_day(e.created_at_ms, tz)).or_default() += w;
        let lang = e.language.clone().unwrap_or_else(|| "?".into());
        *langs.entry(lang).or_default() += w;
    }

    let total_dictations = entries.len() as u64;
    let active_days = per_day.len() as f64;
    let total_min = total_ms as f64 / 60_000.0;

    let week_start = today - Duration::days(6);
    let words_week = per_day.range(week_start..=today).map(|(_, w)| *w).sum();

    let days: BTreeSet<NaiveDate> = per_day.keys().copied().collect();
    let (current_streak, best_streak) = streaks(&days, today);

    let last_30_days = (0..30)
        .rev()
        .map(|back| {
            let d = today - Duration::days(back);
            (d.format("%Y-%m-%d").to_string(), per_day.get(&d).copied().unwrap_or(0))
        })
        .collect();

    let mut languages: Vec<(String, u64)> = langs.into_iter().collect();
    languages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    Stats {
        total_words,
        words_today: per_day.get(&today).copied().unwrap_or(0),
        words_week,
        total_dictations,
        avg_words_per_day: if active_days > 0.0 { total_words as f64 / active_days } else { 0.0 },
        avg_words_per_dictation: if total_dictations > 0 {
            total_words as f64 / total_dictations as f64
        } else {
            0.0
        },
        avg_wpm: if total_min > 0.0 { total_words as f64 / total_min } else { 0.0 },
        current_streak,
        best_streak,
        time_saved_min: (total_words as f64 / TYPING_WPM - total_min).max(0.0),
        last_30_days,
        languages,
    }
}

/// Current streak counts consecutive active days ending today, or ending yesterday
/// when nothing was dictated yet today (the streak is not broken until the day ends).
fn streaks(days: &BTreeSet<NaiveDate>, today: NaiveDate) -> (u32, u32) {
    let mut best = 0u32;
    let mut run = 0u32;
    let mut prev: Option<NaiveDate> = None;
    for d in days {
        run = match prev {
            Some(p) if *d - p == Duration::days(1) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(*d);
    }

    let mut cursor = if days.contains(&today) { today } else { today - Duration::days(1) };
    let mut current = 0u32;
    while days.contains(&cursor) {
        current += 1;
        cursor -= Duration::days(1);
    }
    (current, best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn tz() -> FixedOffset {
        FixedOffset::east_opt(2 * 3600).unwrap() // GMT+2
    }

    /// Unix ms for a local (GMT+2) date-time.
    fn at(y: i32, m: u32, d: u32, h: u32) -> i64 {
        tz().with_ymd_and_hms(y, m, d, h, 0, 0).unwrap().timestamp_millis()
    }

    fn entry(ms: i64, words: u32, dur_ms: i64, lang: &str) -> Entry {
        Entry { created_at_ms: ms, duration_ms: dur_ms, words, language: Some(lang.into()) }
    }

    #[test]
    fn counts_words_in_spanish_and_ignores_symbols() {
        assert_eq!(count_words("Hola, ¿cómo estás?"), 3);
        assert_eq!(count_words("  ... — 🙂 "), 0);
        assert_eq!(count_words("e-mail test123"), 2);
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn empty_history_is_all_zero() {
        let s = compute(&[], at(2026, 9, 28, 12), &tz());
        assert_eq!(s.total_words, 0);
        assert_eq!(s.avg_wpm, 0.0);
        assert_eq!(s.current_streak, 0);
        assert_eq!(s.last_30_days.len(), 30);
        assert_eq!(s.last_30_days.last().unwrap().0, "2026-09-28");
    }

    #[test]
    fn totals_averages_and_wpm() {
        let e = vec![
            entry(at(2026, 9, 27, 10), 60, 30_000, "es"),
            entry(at(2026, 9, 28, 9), 40, 15_000, "en"),
            entry(at(2026, 9, 28, 11), 20, 15_000, "es"),
        ];
        let s = compute(&e, at(2026, 9, 28, 12), &tz());
        assert_eq!(s.total_words, 120);
        assert_eq!(s.words_today, 60);
        assert_eq!(s.total_dictations, 3);
        assert_eq!(s.avg_words_per_day, 60.0);
        assert_eq!(s.avg_words_per_dictation, 40.0);
        assert_eq!(s.avg_wpm, 120.0); // 120 words in 60 s
        assert_eq!(s.languages, vec![("es".into(), 80), ("en".into(), 40)]);
        assert!((s.time_saved_min - 2.0).abs() < 1e-9); // 120/40 = 3 min typing - 1 min talking
    }

    #[test]
    fn buckets_by_local_midnight() {
        // 23:30 local on the 27th is 21:30 UTC; 00:30 local on the 28th is 22:30 UTC the 27th.
        let late = tz().with_ymd_and_hms(2026, 9, 27, 23, 30, 0).unwrap().timestamp_millis();
        let early = tz().with_ymd_and_hms(2026, 9, 28, 0, 30, 0).unwrap().timestamp_millis();
        let s = compute(&[entry(late, 5, 1000, "es"), entry(early, 7, 1000, "es")], at(2026, 9, 28, 12), &tz());
        assert_eq!(s.words_today, 7);
        assert_eq!(s.last_30_days[28], ("2026-09-27".into(), 5));
    }

    #[test]
    fn streaks_count_consecutive_days_and_reset_on_gaps() {
        let e = vec![
            entry(at(2026, 9, 20, 10), 1, 1000, "es"),
            entry(at(2026, 9, 21, 10), 1, 1000, "es"),
            entry(at(2026, 9, 22, 10), 1, 1000, "es"),
            entry(at(2026, 9, 22, 11), 1, 1000, "es"),
            // gap on 23
            entry(at(2026, 9, 24, 10), 1, 1000, "es"),
            entry(at(2026, 9, 25, 10), 1, 1000, "es"),
        ];
        let s = compute(&e, at(2026, 9, 25, 20), &tz());
        assert_eq!(s.current_streak, 2);
        assert_eq!(s.best_streak, 3);
    }

    #[test]
    fn streak_survives_until_today_ends() {
        let e = vec![entry(at(2026, 9, 26, 10), 1, 1000, "es"), entry(at(2026, 9, 27, 10), 1, 1000, "es")];
        assert_eq!(compute(&e, at(2026, 9, 28, 8), &tz()).current_streak, 2);
        assert_eq!(compute(&e, at(2026, 9, 29, 8), &tz()).current_streak, 0);
    }
}
