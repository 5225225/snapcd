use crate::KeyBuf;

use failure::Fallible;
use std::path::Path;
use std::borrow::Cow;
use rusqlite::{params, OptionalExtension};

#[derive(Copy, Clone)]
pub struct CacheKey {
    pub inode: u64,
    pub mtime: i64,
    pub size: u64,
}

pub trait Cache {
    fn raw_get(&self, cachekey: &[u8]) -> Fallible<Option<Vec<u8>>>;
    fn raw_put(&self, cachekey: &[u8], value: &[u8]) -> Fallible<()>;

    fn get(&self, cachekey: CacheKey) -> Fallible<Option<KeyBuf>> {
        let mut data = Vec::with_capacity(8*3);
        data.extend(cachekey.inode.to_le_bytes().iter());
        data.extend(cachekey.mtime.to_le_bytes().iter());
        data.extend(cachekey.size.to_le_bytes().iter());

        let cache_result = self.raw_get(&data)?;

        Ok(cache_result.map(|x| KeyBuf::from_db_key(&x)))
    }

    fn put(&self, cachekey: CacheKey, value: &KeyBuf) {
        let mut data = Vec::with_capacity(8*3);

        data.extend(cachekey.inode.to_le_bytes().iter());
        data.extend(cachekey.mtime.to_le_bytes().iter());
        data.extend(cachekey.size.to_le_bytes().iter());

        self.raw_put(&data, &value.as_db_key());
    }
}

pub struct SqliteCache {
    conn: rusqlite::Connection,
}

impl SqliteCache {
    pub fn new(path: impl AsRef<Path>) -> Fallible<Self> {
        let conn = rusqlite::Connection::open(path)?;

        conn.pragma_update(None, &"journal_mode", &"WAL")?;

        // It's a cache. Speed is more important than safety.
        conn.pragma_update(None, &"synchronous", &"OFF")?;


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
fn raw_get<'a>(&'a self, key: &[u8]) -> Fallible<Option<Vec<u8>>> {
    let results: Result<Option<Vec<u8>>, _> = self.conn.query_row(
        "SELECT value FROM cache WHERE key=?",
        params![key],
        |row| row.get(0),
    ).optional();

    Ok(results?)
}

fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
    self.conn.execute(
        "INSERT OR IGNORE INTO cache VALUES (?, ?)",
        params![key, data],
    )?;

    Ok(())
}

}
