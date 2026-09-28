//! SQLite history of every dictation. One table, see spec §6.

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

    /// Newest first. `query` filters by a case-insensitive substring of the text.
    pub fn list(&self, query: Option<&str>, limit: u32, offset: u32) -> Result<Vec<Dictation>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query.unwrap_or("").trim());
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM dictations WHERE text LIKE ?1
             ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![pattern, limit, offset], row_to_dictation)?;
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
        s.delete(a.id).unwrap();
        assert!(s.get(a.id).unwrap().is_none());
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
