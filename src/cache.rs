use crate::ds::ToDSErrorResult;
use rusqlite::{params, OptionalExtension};
use std::path::Path;
use thiserror::Error;

use crate::{ds, key};

#[derive(Copy, Clone, Debug)]
pub struct CacheKey {
    pub inode: u64,
    pub mtime: i64,
    pub size: u64,
}

#[derive(Debug, Error)]
pub enum RawGetCacheError {
    #[error("data store error: {_0}")]
    DSError(#[from] ds::DSError),
}

#[derive(Debug, Error)]
pub enum RawPutCacheError {
    #[error("data store error: {_0}")]
    DSError(#[from] ds::DSError),
}

#[derive(Debug, Error)]
pub enum PutCacheError {
    #[error("error when inserting item into cache: {_0}")]
    RawPutCacheError(#[from] RawPutCacheError),
}

#[derive(Debug, Error)]
pub enum GetCacheError {
    #[error("error when getting item from cache: {_0}")]
    RawGetCacheError(#[from] RawGetCacheError),

    #[error("error when parsing key from cache: {_0}")]
    FromDbKeyError(#[from] key::FromDbKeyError),
}

pub trait Cache: ds::Transactional {
    fn raw_get(&self, cachekey: &[u8]) -> Result<Option<Vec<u8>>, RawGetCacheError>;
    fn raw_put(&self, cachekey: &[u8], value: &[u8]) -> Result<(), RawPutCacheError>;

    fn get(&self, cachekey: CacheKey) -> Result<Option<key::Key>, GetCacheError> {
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

    fn put(&self, cachekey: CacheKey, value: key::Key) -> Result<(), PutCacheError> {
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
    pub fn new(path: impl AsRef<Path>) -> Result<Self, NewSqliteCacheError> {
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

impl ds::Transactional for SqliteCache {
    fn begin_trans(&mut self) -> Result<(), ds::BeginTransError> {
        self.conn
            .execute("BEGIN TRANSACTION", params![])
            .into_ds_r()?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), ds::CommitTransError> {
        self.conn.execute("COMMIT", params![]).into_ds_r()?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), ds::RollbackTransError> {
        self.conn.execute("ROLLBACK", params![]).into_ds_r()?;
        Ok(())
    }
}

impl Cache for SqliteCache {
    fn raw_get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RawGetCacheError> {
        let results: Result<Option<Vec<u8>>, _> = self
            .conn
            .query_row("SELECT value FROM cache WHERE key=?", params![key], |row| {
                row.get(0)
            })
            .optional();

        Ok(results.into_ds_r()?)
    }

    fn raw_put(&self, key: &[u8], data: &[u8]) -> Result<(), RawPutCacheError> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO cache VALUES (?, ?)",
                params![key, data],
            )
            .into_ds_r()?;

        Ok(())
    }
}
