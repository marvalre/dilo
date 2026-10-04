//! SQLite history of every dictation. One table, see spec §6.

use crate::rules::{Rule, RuleKind};
use crate::stats::{self, count_words, Stats};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Dictation {
    pub id: i64,
    pub created_at: i64,
    pub duration_ms: i64,
    pub text: String,
    pub word_count: u32,
    pub language: Option<String>,
    pub app_name: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NewDictation {
    pub created_at: i64,
    pub duration_ms: i64,
    pub text: String,
    pub language: Option<String>,
    pub app_name: Option<String>,
    pub error: Option<String>,
}

/// Lowercase without accents: "Reunión MAÑANA" → "reunion manana".
/// Also drops combining marks, so decomposed text (e + U+0301) folds like precomposed text.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !matches!(*c as u32, 0x0300..=0x036F))
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'ā' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'ē' | 'ę' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'ī' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' | 'ō' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'ū' => 'u',
            'ñ' | 'ń' => 'n',
            'ç' | 'ć' | 'č' => 'c',
            'ý' | 'ÿ' => 'y',
            'ś' | 'š' => 's',
            'ź' | 'ż' | 'ž' => 'z',
            'ł' => 'l',
            other => other,
        })
        .collect()
}

/// True when SQLite says the file is not a usable database.
fn is_corrupt(e: &anyhow::Error) -> bool {
    use rusqlite::ffi::ErrorCode;
    matches!(
        e.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(f, _)) if matches!(f.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
    )
}

/// Cut-off timestamp for "older than N days"; saturates instead of overflowing.
pub fn cutoff_ms(now_ms: i64, days: u32) -> i64 {
    now_ms.saturating_sub((days as i64).saturating_mul(86_400_000))
}

/// Total size in bytes of the files under `path` (0 if it doesn't exist). Symlinks are not followed.
pub fn dir_size(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .fold(0u64, u64::saturating_add)
        })
        .unwrap_or(0)
}

pub struct Store {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS dictations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_at INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,
  text TEXT NOT NULL,
  word_count INTEGER NOT NULL,
  language TEXT,
  app_name TEXT,
  error TEXT
);
CREATE INDEX IF NOT EXISTS idx_dictations_created ON dictations(created_at);
CREATE TABLE IF NOT EXISTS rules (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,
  src TEXT NOT NULL,
  dst TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1
);
";

const COLUMNS: &str = "id, created_at, duration_ms, text, word_count, language, app_name, error";

fn row_to_dictation(r: &rusqlite::Row) -> rusqlite::Result<Dictation> {
    Ok(Dictation {
        id: r.get(0)?,
        created_at: r.get(1)?,
        duration_ms: r.get(2)?,
        text: r.get(3)?,
        word_count: r.get::<_, i64>(4)?.clamp(0, u32::MAX as i64) as u32,
        language: r.get(5)?,
        app_name: r.get(6)?,
        error: r.get(7)?,
    })
}

impl Store {
    /// Opens (or creates) the database. A file that is not a database (corrupted, truncated, overwritten)
    /// is moved aside as `<name>.corrupt-<unix seconds>` and a fresh database is created, so the app
    /// still starts and the old file can be recovered by hand.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)?;
            }
        }
        match Connection::open(path).map_err(anyhow::Error::from).and_then(Self::init) {
            Ok(store) => Ok(store),
            Err(e) if is_corrupt(&e) => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let mut aside = path.as_os_str().to_owned();
                aside.push(format!(".corrupt-{ts}"));
                log::error!("database unreadable ({e}); moving it to {aside:?} and starting fresh");
                std::fs::rename(path, &aside)?;
                for ext in ["-wal", "-shm", "-journal"] {
                    let mut side = path.as_os_str().to_owned();
                    side.push(ext);
                    let _ = std::fs::remove_file(side);
                }
                Self::init(Connection::open(path)?)
            }
            Err(e) => Err(e),
        }
    }

    /// Locks the connection. A panic in another thread while holding the lock must not
    /// make every later call panic too: SQLite connections stay consistent across it.
    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // Another instance (or a slow VACUUM) holding the file must not turn into instant errors.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        // Schema version marker for future migrations (a newer version is left untouched).
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            conn.execute_batch("PRAGMA user_version = 1")?;
        }
        conn.create_scalar_function(
            "fold",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8 | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| Ok(fold(&ctx.get::<Option<String>>(0)?.unwrap_or_default())),
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn insert(&self, d: NewDictation) -> Result<Dictation> {
        let words = if d.error.is_some() { 0 } else { count_words(&d.text) };
        let conn = self.conn();
        conn.execute(
            "INSERT INTO dictations (created_at, duration_ms, text, word_count, language, app_name, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![d.created_at, d.duration_ms, d.text, words, d.language, d.app_name, d.error],
        )?;
        let id = conn.last_insert_rowid();
        Ok(conn.query_row(
            &format!("SELECT {COLUMNS} FROM dictations WHERE id = ?1"),
            [id],
            row_to_dictation,
        )?)
    }

    /// Newest first. `query` filters by a substring, ignoring case and accents.
    pub fn list(&self, query: Option<&str>, limit: u32, offset: u32) -> Result<Vec<Dictation>> {
        let conn = self.conn();
        let needle = fold(query.unwrap_or("").trim());
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM dictations WHERE ?1 = '' OR instr(fold(text), ?1) > 0
             ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![needle, limit, offset], row_to_dictation)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get(&self, id: i64) -> Result<Option<Dictation>> {
        let conn = self.conn();
        Ok(conn
            .query_row(&format!("SELECT {COLUMNS} FROM dictations WHERE id = ?1"), [id], row_to_dictation)
            .optional()?)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn().execute("DELETE FROM dictations WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Replaces a dictation's text (user edit) and recomputes its word count.
    pub fn update_text(&self, id: i64, text: &str) -> Result<Option<Dictation>> {
        self.conn().execute(
            "UPDATE dictations SET text = ?1, word_count = ?2, error = NULL WHERE id = ?3",
            params![text, count_words(text), id],
        )?;
        self.get(id)
    }

    /// Deletes everything, or only rows created before `before_ms`. Returns rows deleted.
    pub fn clear(&self, before_ms: Option<i64>) -> Result<usize> {
        let n = {
            let conn = self.conn();
            match before_ms {
                Some(ms) => conn.execute("DELETE FROM dictations WHERE created_at < ?1", [ms])?,
                None => conn.execute("DELETE FROM dictations", [])?,
            }
        };
        // Give the space back to the disk; best effort (needs free space, may be slow).
        if n > 0 {
            if let Err(e) = self.conn().execute_batch("VACUUM") {
                log::warn!("vacuum: {e}");
            }
        }
        Ok(n)
    }

    pub fn count(&self) -> Result<u64> {
        Ok(self.conn().query_row("SELECT COUNT(*) FROM dictations", [], |r| r.get(0))?)
    }

    /// Size of the database in bytes (pages in use × page size).
    pub fn size_bytes(&self) -> Result<u64> {
        let conn = self.conn();
        let pages: u64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let size: u64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(pages * size)
    }

    pub fn rules(&self) -> Result<Vec<Rule>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, kind, src, dst, enabled FROM rules ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| {
            let kind: String = r.get(1)?;
            Ok(Rule {
                id: r.get(0)?,
                kind: RuleKind::parse(&kind).unwrap_or(RuleKind::Correction),
                from: r.get(2)?,
                to: r.get(3)?,
                enabled: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Inserts when `id` is None, updates otherwise. Returns the stored rule.
    pub fn save_rule(&self, id: Option<i64>, kind: RuleKind, from: &str, to: &str, enabled: bool) -> Result<Rule> {
        crate::rules::validate(from, to).map_err(|e| anyhow::anyhow!(e))?;
        let id = {
            let conn = self.conn();
            match id {
                Some(id) => {
                    conn.execute(
                        "UPDATE rules SET kind = ?1, src = ?2, dst = ?3, enabled = ?4 WHERE id = ?5",
                        params![kind.as_str(), from.trim(), to, enabled, id],
                    )?;
                    id
                }
                None => {
                    conn.execute(
                        "INSERT INTO rules (kind, src, dst, enabled) VALUES (?1, ?2, ?3, ?4)",
                        params![kind.as_str(), from.trim(), to, enabled],
                    )?;
                    conn.last_insert_rowid()
                }
            }
        };
        self.rules()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("rule {id} not found"))
    }

    pub fn delete_rule(&self, id: i64) -> Result<()> {
        self.conn().execute("DELETE FROM rules WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Stats in the user's local time zone. Failed dictations are excluded.
    pub fn stats(&self, now_ms: i64) -> Result<Stats> {
        Ok(stats::compute(&self.entries()?, now_ms, &chrono::Local))
    }

    fn entries(&self) -> Result<Vec<stats::Entry>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT created_at, duration_ms, word_count, language FROM dictations WHERE error IS NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(stats::Entry {
                created_at_ms: r.get(0)?,
                duration_ms: r.get(1)?,
                words: r.get::<_, i64>(2)?.clamp(0, u32::MAX as i64) as u32,
                language: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(text: &str, at: i64) -> NewDictation {
        NewDictation {
            created_at: at,
            duration_ms: 2000,
            text: text.into(),
            language: Some("es".into()),
            ..Default::default()
        }
    }

    #[test]
    fn insert_computes_word_count_and_list_is_newest_first() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert(new("hola mundo", 1_000)).unwrap();
        s.insert(new("uno dos tres", 2_000)).unwrap();
        assert_eq!(a.word_count, 2);
        let all = s.list(None, 50, 0).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].text, "uno dos tres");
    }

    #[test]
    fn search_is_case_insensitive_and_delete_removes() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert(new("Reunión con Ana", 1_000)).unwrap();
        s.insert(new("comprar leche", 2_000)).unwrap();
        assert_eq!(s.list(Some("reunión con"), 50, 0).unwrap().len(), 1);
        assert_eq!(s.list(Some("LECHE"), 50, 0).unwrap().len(), 1);
        assert_eq!(s.list(Some("REUNIÓN"), 50, 0).unwrap().len(), 1);
        assert_eq!(s.list(Some("reunion con ana"), 50, 0).unwrap().len(), 1);
        assert_eq!(s.list(Some("_"), 50, 0).unwrap().len(), 0);
        assert_eq!(s.list(Some("%"), 50, 0).unwrap().len(), 0);
        assert_eq!(s.list(Some("  "), 50, 0).unwrap().len(), 2);
        s.delete(a.id).unwrap();
        assert!(s.get(a.id).unwrap().is_none());
    }

    #[test]
    fn edit_clear_and_rules() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert(new("hola", 1_000)).unwrap();
        s.insert(new("viejo", 500)).unwrap();
        let edited = s.update_text(a.id, "hola mundo bonito").unwrap().unwrap();
        assert_eq!((edited.text.as_str(), edited.word_count), ("hola mundo bonito", 3));
        assert_eq!(s.clear(Some(900)).unwrap(), 1);
        assert_eq!(s.count().unwrap(), 1);
        assert!(s.size_bytes().unwrap() > 0);

        let r = s.save_rule(None, RuleKind::Shortcut, " mi correo ", "a@x.com", true).unwrap();
        assert_eq!(r.from, "mi correo");
        let r2 = s.save_rule(Some(r.id), RuleKind::Shortcut, "mi correo", "b@x.com", false).unwrap();
        assert_eq!((r2.to.as_str(), r2.enabled), ("b@x.com", false));
        s.delete_rule(r.id).unwrap();
        assert!(s.rules().unwrap().is_empty());
        assert_eq!(s.clear(None).unwrap(), 1);
    }

    #[test]
    fn stats_exclude_failed_dictations() {
        let s = Store::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        s.insert(new("uno dos tres cuatro", now)).unwrap();
        s.insert(NewDictation { error: Some("engine failed".into()), ..new("", now) }).unwrap();
        let st = s.stats(now).unwrap();
        assert_eq!(st.total_words, 4);
        assert_eq!(st.total_dictations, 1);
        assert_eq!(st.words_today, 4);
    }

    #[test]
    fn sql_special_characters_are_data_not_code() {
        let s = Store::open_in_memory().unwrap();
        let evil = "x'); DROP TABLE dictations; -- \" \\ %_ \0 end";
        s.insert(new(evil, 1)).unwrap();
        s.insert(new("normal", 2)).unwrap();
        assert_eq!(s.list(Some("'); drop table"), 50, 0).unwrap().len(), 1);
        assert_eq!(s.list(Some("\\"), 50, 0).unwrap().len(), 1);
        assert_eq!(s.count().unwrap(), 2);
        let r = s.save_rule(None, RuleKind::Correction, "a'b\"c", "'; DELETE FROM rules; --", true).unwrap();
        assert_eq!(r.to, "'; DELETE FROM rules; --");
        assert_eq!(s.rules().unwrap().len(), 1);
    }

    #[test]
    fn fold_handles_decomposed_accents_and_unicode_case() {
        assert_eq!(fold("Cafe\u{301} NIN\u{303}O"), "cafe nino");
        assert_eq!(fold("ÁÉÍÓÚ Ñ Ü"), "aeiou n u");
        let s = Store::open_in_memory().unwrap();
        s.insert(new("reunio\u{301}n", 1)).unwrap();
        assert_eq!(s.list(Some("REUNIÓN"), 50, 0).unwrap().len(), 1);
        s.insert(new("你好世界 🙂", 2)).unwrap();
        assert_eq!(s.list(Some("世界"), 50, 0).unwrap().len(), 1);
    }

    #[test]
    fn pagination_and_extreme_limits() {
        let s = Store::open_in_memory().unwrap();
        for i in 0..5 {
            s.insert(new(&format!("n{i}"), i)).unwrap();
        }
        assert_eq!(s.list(None, 2, 0).unwrap().len(), 2);
        assert_eq!(s.list(None, 2, 4).unwrap().len(), 1);
        assert_eq!(s.list(None, u32::MAX, u32::MAX).unwrap().len(), 0);
        assert_eq!(s.list(None, 0, 0).unwrap().len(), 0);
    }

    #[test]
    fn empty_and_huge_text_and_missing_ids() {
        let s = Store::open_in_memory().unwrap();
        let d = s.insert(new("", 1)).unwrap();
        assert_eq!(d.word_count, 0);
        let big = "palabra ".repeat(200_000);
        let d2 = s.insert(new(&big, 2)).unwrap();
        assert_eq!(d2.word_count, 200_000);
        assert!(s.update_text(9999, "x").unwrap().is_none());
        s.delete(9999).unwrap();
        s.delete_rule(9999).unwrap();
        assert!(s.save_rule(Some(9999), RuleKind::Correction, "a", "b", true).is_err());
        assert!(s.save_rule(None, RuleKind::Correction, "   ", "b", true).is_err());
        assert_eq!(s.clear(Some(i64::MIN)).unwrap(), 0);
    }

    #[test]
    fn corrupt_database_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("dilo.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"this is definitely not an sqlite database, just garbage bytes!!!!").unwrap();
        let s = Store::open(&path).unwrap();
        s.insert(new("hola", 1)).unwrap();
        assert_eq!(s.count().unwrap(), 1);
        let aside = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(aside, 1);
        drop(s);
        // Reopening keeps the data.
        assert_eq!(Store::open(&path).unwrap().count().unwrap(), 1);
    }

    #[test]
    fn bad_rows_do_not_break_listing_or_stats() {
        let s = Store::open_in_memory().unwrap();
        {
            let c = s.conn();
            c.execute("INSERT INTO dictations (created_at, duration_ms, text, word_count) VALUES (1, 1, 'a', -7)", []).unwrap();
            c.execute("INSERT INTO rules (kind, src, dst) VALUES ('bogus', 'x', 'y')", []).unwrap();
        }
        assert_eq!(s.list(None, 10, 0).unwrap()[0].word_count, 0);
        assert_eq!(s.stats(1).unwrap().total_words, 0);
        assert_eq!(s.rules().unwrap()[0].kind, RuleKind::Correction);
    }

    #[test]
    fn poisoned_lock_does_not_cascade() {
        let s = std::sync::Arc::new(Store::open_in_memory().unwrap());
        let s2 = s.clone();
        let _ = std::thread::spawn(move || {
            let _g = s2.conn();
            panic!("boom");
        })
        .join();
        assert_eq!(s.count().unwrap(), 0);
    }

    #[test]
    fn concurrent_inserts_are_all_stored() {
        let s = std::sync::Arc::new(Store::open_in_memory().unwrap());
        let hs: Vec<_> = (0..8)
            .map(|t| {
                let s = s.clone();
                std::thread::spawn(move || {
                    for i in 0..50 {
                        s.insert(new(&format!("t{t} {i}"), i)).unwrap();
                        let _ = s.list(Some("t"), 10, 0).unwrap();
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(s.count().unwrap(), 400);
    }

    #[test]
    fn cutoff_and_dir_size() {
        assert_eq!(cutoff_ms(1_000_000_000_000, 1), 1_000_000_000_000 - 86_400_000);
        assert_eq!(cutoff_ms(i64::MIN + 5, u32::MAX), i64::MIN);
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("a")).unwrap();
        std::fs::write(dir.path().join("a/x"), [0u8; 10]).unwrap();
        std::fs::write(dir.path().join("y"), [0u8; 5]).unwrap();
        assert_eq!(dir_size(dir.path()), 15);
        assert_eq!(dir_size(&dir.path().join("missing")), 0);
    }
}
