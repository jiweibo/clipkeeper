//! SQLite-backed clipboard history.
//!
//! Text is stored inline. Images are stored as PNG files under `images/`
//! next to the database, and the row keeps the file name.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
}

impl ClipKind {
    fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
        }
    }

    fn parse(s: &str) -> Self {
        if s == "image" {
            ClipKind::Image
        } else {
            ClipKind::Text
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub kind: ClipKind,
    /// Full text for text clips; empty for images.
    pub text: String,
    /// Absolute path to the PNG for image clips.
    pub image_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pinned: bool,
    pub created_at: i64,
}

/// What the clipboard watcher hands to the store.
pub enum NewClip {
    Text(String),
    Image {
        png: Vec<u8>,
        width: u32,
        height: u32,
        /// Hash of the raw pixels, so the same picture copied twice is
        /// recognised even if PNG encoding differs.
        key: String,
    },
}

pub struct Store {
    conn: Mutex<Connection>,
    image_dir: PathBuf,
    max_items: AtomicUsize,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

impl Store {
    pub fn open(data_dir: &Path, max_items: usize) -> rusqlite::Result<Self> {
        let image_dir = data_dir.join("images");
        let _ = std::fs::create_dir_all(&image_dir);
        let conn = Connection::open(data_dir.join("history.db"))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS clips (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 kind       TEXT    NOT NULL,
                 text       TEXT    NOT NULL DEFAULT '',
                 image_file TEXT,
                 width      INTEGER,
                 height     INTEGER,
                 hash       TEXT    NOT NULL UNIQUE,
                 pinned     INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS clips_order ON clips (pinned DESC, created_at DESC);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
            image_dir,
            max_items: AtomicUsize::new(max_items),
        })
    }

    /// Saves a clip, or bumps an identical existing one to the top.
    /// Returns the clip's id.
    pub fn insert(&self, clip: NewClip) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        let (kind, text, png, width, height, hash) = match clip {
            NewClip::Text(t) => {
                let hash = hash_bytes(t.as_bytes());
                (ClipKind::Text, t, None, None, None, hash)
            }
            NewClip::Image { png, width, height, key } => {
                (ClipKind::Image, String::new(), Some(png), Some(width), Some(height), key)
            }
        };
        // Strictly increasing, so copies made within the same millisecond
        // still keep their order.
        let latest: i64 =
            conn.query_row("SELECT COALESCE(MAX(created_at), 0) FROM clips", [], |r| r.get(0))?;
        let ts = now_ms().max(latest + 1);

        let existing: Option<i64> = conn
            .query_row("SELECT id FROM clips WHERE hash = ?1", [&hash], |r| r.get(0))
            .optional()?;
        if let Some(id) = existing {
            conn.execute("UPDATE clips SET created_at = ?1 WHERE id = ?2", params![ts, id])?;
            return Ok(id);
        }

        let image_file = match png {
            Some(bytes) => {
                let name = format!("{hash}.png");
                if let Err(e) = std::fs::write(self.image_dir.join(&name), bytes) {
                    eprintln!("failed to save image: {e}");
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Some(name)
            }
            None => None,
        };
        conn.execute(
            "INSERT INTO clips (kind, text, image_file, width, height, hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![kind.as_str(), text, image_file, width, height, hash, ts],
        )?;
        let id = conn.last_insert_rowid();
        self.prune(&conn)?;
        Ok(id)
    }

    /// Lists clips, pinned first, newest first. `query` filters text clips.
    pub fn list(&self, query: &str, limit: usize) -> rusqlite::Result<Vec<Clip>> {
        let conn = self.conn.lock().unwrap();
        let q = query.trim();
        let mut stmt = conn.prepare(
            "SELECT id, kind, text, image_file, width, height, pinned, created_at
             FROM clips
             WHERE ?1 = '' OR (kind = 'text' AND text LIKE '%' || ?1 || '%' ESCAPE '\\')
             ORDER BY pinned DESC, created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        let rows = stmt.query_map(params![escaped, limit as i64], |r| self.row_to_clip(r))?;
        rows.collect()
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Option<Clip>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, kind, text, image_file, width, height, pinned, created_at
             FROM clips WHERE id = ?1",
            [id],
            |r| self.row_to_clip(r),
        )
        .optional()
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE clips SET pinned = ?1 WHERE id = ?2", params![pinned, id])?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        let file: Option<Option<String>> = conn
            .query_row("SELECT image_file FROM clips WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
        if let Some(Some(name)) = file {
            let _ = std::fs::remove_file(self.image_dir.join(name));
        }
        Ok(())
    }

    /// Changes how many unpinned clips are kept, dropping the oldest ones
    /// right away if the history is now over the limit.
    pub fn set_max_items(&self, max_items: usize) -> rusqlite::Result<()> {
        self.max_items.store(max_items, Ordering::Relaxed);
        let conn = self.conn.lock().unwrap();
        self.prune(&conn)
    }

    /// Removes every clip that is not pinned.
    pub fn clear_unpinned(&self) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        self.remove_where(&conn, "pinned = 0", [])
    }

    /// Keeps at most `max_items` unpinned clips.
    fn prune(&self, conn: &Connection) -> rusqlite::Result<()> {
        self.remove_where(
            conn,
            "pinned = 0 AND id NOT IN (
                 SELECT id FROM clips WHERE pinned = 0 ORDER BY created_at DESC, id DESC LIMIT ?1
             )",
            [self.max_items.load(Ordering::Relaxed) as i64],
        )
    }

    fn remove_where<P: rusqlite::Params + Clone>(
        &self,
        conn: &Connection,
        cond: &str,
        p: P,
    ) -> rusqlite::Result<()> {
        let mut stmt =
            conn.prepare(&format!("SELECT image_file FROM clips WHERE image_file IS NOT NULL AND {cond}"))?;
        let files: Vec<String> = stmt
            .query_map(p.clone(), |r| r.get(0))?
            .filter_map(Result::ok)
            .collect();
        conn.execute(&format!("DELETE FROM clips WHERE {cond}"), p)?;
        for f in files {
            let _ = std::fs::remove_file(self.image_dir.join(f));
        }
        Ok(())
    }

    fn row_to_clip(&self, r: &rusqlite::Row) -> rusqlite::Result<Clip> {
        let image_file: Option<String> = r.get(3)?;
        Ok(Clip {
            id: r.get(0)?,
            kind: ClipKind::parse(&r.get::<_, String>(1)?),
            text: r.get(2)?,
            image_path: image_file.map(|f| self.image_dir.join(f).to_string_lossy().into_owned()),
            width: r.get(4)?,
            height: r.get(5)?,
            pinned: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(max: usize) -> (Store, PathBuf) {
        let dir = std::env::temp_dir().join(format!("clipkeeper-test-{}", now_ms() ^ rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        (Store::open(&dir, max).unwrap(), dir)
    }

    fn rand_suffix() -> i64 {
        static N: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + std::process::id() as i64 * 1000
    }

    #[test]
    fn dedups_and_bumps() {
        let (s, _) = store(10);
        let a = s.insert(NewClip::Text("hello".into())).unwrap();
        s.insert(NewClip::Text("world".into())).unwrap();
        let again = s.insert(NewClip::Text("hello".into())).unwrap();
        assert_eq!(a, again);
        let list = s.list("", 10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].text, "hello");
    }

    #[test]
    fn prunes_unpinned_only() {
        let (s, _) = store(2);
        let first = s.insert(NewClip::Text("1".into())).unwrap();
        s.set_pinned(first, true).unwrap();
        for t in ["2", "3", "4"] {
            s.insert(NewClip::Text(t.into())).unwrap();
        }
        let texts: Vec<_> = s.list("", 10).unwrap().into_iter().map(|c| c.text).collect();
        assert_eq!(texts, vec!["1", "4", "3"]);
    }

    #[test]
    fn keeps_order_within_same_millisecond() {
        let (s, _) = store(100);
        for i in 0..50 {
            s.insert(NewClip::Text(i.to_string())).unwrap();
        }
        s.insert(NewClip::Text("7".into())).unwrap();
        let texts: Vec<_> = s.list("", 3).unwrap().into_iter().map(|c| c.text).collect();
        assert_eq!(texts, vec!["7", "49", "48"]);
    }

    #[test]
    fn lowering_the_limit_prunes_now() {
        let (s, _) = store(10);
        for i in 0..6 {
            s.insert(NewClip::Text(i.to_string())).unwrap();
        }
        s.set_max_items(2).unwrap();
        let texts: Vec<_> = s.list("", 10).unwrap().into_iter().map(|c| c.text).collect();
        assert_eq!(texts, vec!["5", "4"]);
    }

    #[test]
    fn search_escapes_wildcards() {
        let (s, _) = store(10);
        s.insert(NewClip::Text("100% sure".into())).unwrap();
        s.insert(NewClip::Text("100 apples".into())).unwrap();
        assert_eq!(s.list("0%", 10).unwrap().len(), 1);
        assert_eq!(s.list("APPLE", 10).unwrap().len(), 1);
    }

    #[test]
    fn image_file_lifecycle() {
        let (s, _) = store(10);
        let id = s
            .insert(NewClip::Image { png: vec![1, 2, 3], width: 1, height: 1, key: "img".into() })
            .unwrap();
        let path = s.get(id).unwrap().unwrap().image_path.unwrap();
        assert!(Path::new(&path).exists());
        s.delete(id).unwrap();
        assert!(!Path::new(&path).exists());
    }
}
