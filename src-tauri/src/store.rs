//! SQLite history of every dictation. One table, see spec §6.

use crate::rules::{Rule, RuleKind};
use crate::stats::{self, count_words, Stats};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

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
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
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
        word_count: r.get(4)?,
        language: r.get(5)?,
        app_name: r.get(6)?,
        error: r.get(7)?,
    })
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        conn.create_scalar_function(
            "fold",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8 | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| Ok(fold(&ctx.get::<String>(0)?)),
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn insert(&self, d: NewDictation) -> Result<Dictation> {
        let words = if d.error.is_some() { 0 } else { count_words(&d.text) };
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let needle = fold(query.unwrap_or("").trim());
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM dictations WHERE ?1 = '' OR instr(fold(text), ?1) > 0
             ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![needle, limit, offset], row_to_dictation)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get(&self, id: i64) -> Result<Option<Dictation>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row(&format!("SELECT {COLUMNS} FROM dictations WHERE id = ?1"), [id], row_to_dictation)
            .optional()?)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM dictations WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Replaces a dictation's text (user edit) and recomputes its word count.
    pub fn update_text(&self, id: i64, text: &str) -> Result<Option<Dictation>> {
        self.conn.lock().unwrap().execute(
            "UPDATE dictations SET text = ?1, word_count = ?2, error = NULL WHERE id = ?3",
            params![text, count_words(text), id],
        )?;
        self.get(id)
    }

    /// Deletes everything, or only rows created before `before_ms`. Returns rows deleted.
    pub fn clear(&self, before_ms: Option<i64>) -> Result<usize> {
        let n = {
            let conn = self.conn.lock().unwrap();
            match before_ms {
                Some(ms) => conn.execute("DELETE FROM dictations WHERE created_at < ?1", [ms])?,
                None => conn.execute("DELETE FROM dictations", [])?,
            }
        };
        // Give the space back to the disk; best effort (needs free space, may be slow).
        if n > 0 {
            if let Err(e) = self.conn.lock().unwrap().execute_batch("VACUUM") {
                log::warn!("vacuum: {e}");
            }
        }
        Ok(n)
    }

    pub fn count(&self) -> Result<u64> {
        Ok(self.conn.lock().unwrap().query_row("SELECT COUNT(*) FROM dictations", [], |r| r.get(0))?)
    }

    /// Size of the database in bytes (pages in use × page size).
    pub fn size_bytes(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let pages: u64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let size: u64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(pages * size)
    }

    pub fn rules(&self) -> Result<Vec<Rule>> {
        let conn = self.conn.lock().unwrap();
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
        let id = {
            let conn = self.conn.lock().unwrap();
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
        self.conn.lock().unwrap().execute("DELETE FROM rules WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Stats in the user's local time zone. Failed dictations are excluded.
    pub fn stats(&self, now_ms: i64) -> Result<Stats> {
        Ok(stats::compute(&self.entries()?, now_ms, &chrono::Local))
    }

    fn entries(&self) -> Result<Vec<stats::Entry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT created_at, duration_ms, word_count, language FROM dictations WHERE error IS NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(stats::Entry {
                created_at_ms: r.get(0)?,
                duration_ms: r.get(1)?,
                words: r.get(2)?,
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
}
