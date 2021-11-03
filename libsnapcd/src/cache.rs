use rusqlite::{params, OptionalExtension};
use std::path::Path;
use thiserror::Error;

use crate::key;

#[derive(Copy, Clone, Debug)]
pub struct CacheKey {
    pub inode: u64,
    pub mtime: i64,
    pub size: u64,
}

pub trait Cache {
    fn raw_get(&self, cachekey: &[u8]) -> anyhow::Result<Option<Vec<u8>>>;
    fn raw_put(&self, cachekey: &[u8], value: &[u8]) -> anyhow::Result<()>;

    fn get(&self, cachekey: CacheKey) -> anyhow::Result<Option<key::Key>> {
        let mut data = Vec::with_capacity(8 * 3);
        data.extend(cachekey.inode.to_le_bytes().iter());
        data.extend(cachekey.mtime.to_le_bytes().iter());
        data.extend(cachekey.size.to_le_bytes().iter());

        let cache_result = self.raw_get(&data)?;

        match cache_result {
            Some(data) => {
                let key = key::Key::from_db_key(&data)?;
                Ok(Some(key))
            }
            None => Ok(None),
        }
    }

    fn put(&self, cachekey: CacheKey, value: key::Key) -> anyhow::Result<()> {
        let mut data = Vec::with_capacity(8 * 3);

        data.extend(cachekey.inode.to_le_bytes().iter());
        data.extend(cachekey.mtime.to_le_bytes().iter());
        data.extend(cachekey.size.to_le_bytes().iter());

        self.raw_put(&data, &value.as_db_key())?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct SqliteCache {
    conn: rusqlite::Connection,
}

#[derive(Debug, Error)]
pub enum NewSqliteCacheError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
}

impl SqliteCache {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = rusqlite::Connection::open(path)?;

        conn.pragma_update(None, "journal_mode", &"WAL")?;

        conn.pragma_update(None, "synchronous", &"NORMAL")?;

        conn.execute_batch(
            "
                CREATE TABLE IF NOT EXISTS cache (
                    key BLOB NOT NULL UNIQUE PRIMARY KEY,
                    value BLOB NOT NULL
                    ) WITHOUT ROWID;
                ",
        )?;

        Ok(Self { conn })
    }
}

impl Cache for SqliteCache {
    fn raw_get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let results: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT value FROM cache WHERE key=?", params![key], |row| {
                row.get(0)
            })
            .optional()?;

        Ok(results)
    }

    fn raw_put(&self, key: &[u8], data: &[u8]) -> anyhow::Result<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO cache VALUES (?, ?)",
                params![key, data],
            )?;

        Ok(())
    }
}
