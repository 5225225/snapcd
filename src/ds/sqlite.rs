use std::path::Path;

use rusqlite::params;
use rusqlite::OptionalExtension;
use std::borrow::Cow;

use crate::commit;
use crate::ds;
use crate::ds::{
    BeginTransError, CommitTransError, DataStore, GetReflogError, RawBetweenError, RawExistsError,
    RawGetError, RawGetStateError, RawPutError, RawPutStateError, ReflogPushError,
    RollbackTransError, WalkReflogError,
};
use crate::ds::{ToDSError, ToDSErrorResult};
use crate::key::{Key, TypedKey};
use crate::Reflog;
use thiserror::Error;

#[derive(Debug)]
pub struct SqliteDS {
    conn: rusqlite::Connection,
}

#[derive(Debug, Error)]
pub enum NewSqliteError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
}

impl SqliteDS {
    pub fn new<S: AsRef<Path>>(path: S) -> Result<Self, NewSqliteError> {
        let conn = rusqlite::Connection::open(path)?;

        conn.pragma_update(None, &"synchronous", &"2")?;
        conn.pragma_update(None, &"journal_mode", &"truncate")?;
        conn.pragma_update(None, &"page_size", &"16384")?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS data (
                key BLOB NOT NULL UNIQUE PRIMARY KEY,
                value BLOB NOT NULL
            ) WITHOUT ROWID;

            CREATE TABLE IF NOT EXISTS state (
                key BLOB NOT NULL UNIQUE PRIMARY KEY,
                value BLOB NOT NULL
            ) WITHOUT ROWID;

            CREATE TABLE IF NOT EXISTS reflog (
                id INTEGER PRIMARY KEY,
                refname TEXT NOT NULL,
                remote TEXT,
                key BLOB
            );
        ",
        )?;

        Ok(Self { conn })
    }
}

impl ds::Transactional for SqliteDS {
    fn begin_trans(&mut self) -> Result<(), BeginTransError> {
        self.conn
            .execute("BEGIN TRANSACTION", params![])
            .to_ds_r()?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), CommitTransError> {
        self.conn.execute("COMMIT", params![]).to_ds_r()?;

        Ok(())
    }

    fn rollback(&mut self) -> Result<(), RollbackTransError> {
        self.conn.execute("ROLLBACK", params![]).to_ds_r()?;

        Ok(())
    }
}

impl DataStore for SqliteDS {
    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> Result<Key, GetReflogError> {
        log::trace!("reflog_get({:?}, {:?})", refname, remote);

        // We have to use `remote IS ?` here because we want NULL = NULL (it is not remote).
        let query: Result<Option<Vec<u8>>, rusqlite::Error> = self
            .conn
            .query_row(
                "SELECT key FROM reflog WHERE refname=? AND remote IS ? ORDER BY id DESC LIMIT 1",
                params![refname, remote],
                |row| row.get(0),
            )
            .optional();

        match query {
            Ok(Some(k)) => Ok(Key::from_db_key(&k)?.into()),
            Ok(None) => Err(GetReflogError::NotFound),
            Err(e) => Err(e.to_ds().into()),
        }
    }

    fn reflog_push(&self, data: &Reflog) -> Result<(), ReflogPushError> {
        self.conn
            .execute(
                "INSERT INTO reflog(refname, remote, key) VALUES (?, ?, ?)",
                params![data.refname, data.remote, data.key.as_db_key(),],
            )
            .to_ds_r()?;

        Ok(())
    }

    fn reflog_walk(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<Vec<Key>, WalkReflogError> {
        let mut statement = self
            .conn
            .prepare("SELECT key FROM reflog WHERE refname=? AND remote IS ? ORDER BY id DESC")
            .to_ds_r()?;

        let mut rows = statement.query(params![refname, remote]).unwrap();

        let mut keys = Vec::new();

        while let Some(row) = rows.next().unwrap() {
            let buf: Vec<u8> = row.get(0).unwrap();
            keys.push(Key::from_db_key(&buf)?.into());
        }

        Ok(keys)
    }

    fn raw_get<'a>(&'a self, key: &[u8]) -> Result<Cow<'a, [u8]>, RawGetError> {
        let results: Vec<u8> = self
            .conn
            .query_row("SELECT value FROM data WHERE key=?", params![key], |row| {
                row.get(0)
            })
            .to_ds_r()?;

        Ok(Cow::Owned(results))
    }

    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Result<(), RawPutError> {
        self.conn
            .prepare_cached("INSERT OR IGNORE INTO data VALUES (?, ?)")
            .to_ds_r()?
            .execute(params![key, data])
            .to_ds_r()?;

        Ok(())
    }

    fn raw_get_state<'a>(&'a self, key: &[u8]) -> Result<Option<Vec<u8>>, RawGetStateError> {
        let results: Result<Option<Vec<u8>>, _> = self
            .conn
            .query_row("SELECT value FROM state WHERE key=?", params![key], |row| {
                row.get(0)
            })
            .optional();

        Ok(results.to_ds_r()?)
    }

    fn raw_put_state<'a>(&'a self, key: &[u8], data: &[u8]) -> Result<(), RawPutStateError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO state VALUES (?, ?)",
                params![key, data],
            )
            .to_ds_r()?;

        Ok(())
    }

    fn raw_exists(&self, key: &[u8]) -> Result<bool, RawExistsError> {
        let count: u32 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM data WHERE key=?",
                params![key],
                |row| row.get(0),
            )
            .to_ds_r()?;

        assert!(count == 0 || count == 1);

        Ok(count == 1)
    }

    fn raw_between(
        &self,
        start: &[u8],
        end: Option<&[u8]>,
    ) -> Result<Vec<Vec<u8>>, RawBetweenError> {
        log::trace!("raw_between({:?}, {:?})", start, end);

        let mut results = Vec::new();
        if let Some(e) = end {
            let mut statement = self
                .conn
                .prepare("SELECT key FROM data WHERE key >= ? AND key < ?")
                .to_ds_r()?;

            let rows = statement
                .query_map(params![start, e], |row| row.get(0))
                .to_ds_r()?;

            for row in rows {
                results.push(row.to_ds_r()?);
            }
        } else {
            let mut statement = self
                .conn
                .prepare("SELECT key FROM data WHERE key >= ?")
                .to_ds_r()?;
            let rows = statement
                .query_map(params![start], |row| row.get(0))
                .to_ds_r()?;

            for row in rows {
                results.push(row.to_ds_r()?);
            }
        }

        log::trace!("... got results {:?}", &results);
        Ok(results)
    }
}
