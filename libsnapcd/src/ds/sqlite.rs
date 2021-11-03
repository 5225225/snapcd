use std::path::Path;

use rusqlite::params;
use rusqlite::OptionalExtension;
use std::borrow::Cow;

use crate::ds::DataStore;
use crate::ds::Reflog;
use crate::{crypto, key::Key};
use thiserror::Error;

#[derive(Debug)]
pub struct SqliteDs {
    conn: rusqlite::Connection,
    encryption_key: crypto::EncryptionKey,
    gearhash_table: crypto::GearHashTable,
}

#[derive(Debug, Error)]
pub enum NewSqliteError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
}

impl SqliteDs {
    pub fn new<S: AsRef<Path>>(path: S) -> Result<Self, NewSqliteError> {
        let conn = rusqlite::Connection::open(path)?;

        conn.pragma_update(None, "journal_mode", &"WAL")?;
        conn.pragma_update(None, "synchronous", &"1")?;
        conn.pragma_update(None, "page_size", &"16384")?;

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

        let zk = crypto::RepoKey::ZERO;
        let encryption_key = zk.derive_encryption_key();
        let gearhash_table = zk.derive_gearhash_table();

        Ok(Self {
            conn,
            encryption_key,
            gearhash_table,
        })
    }
}

impl DataStore for SqliteDs {
    fn get_encryption_key(&self) -> &crypto::EncryptionKey {
        &self.encryption_key
    }

    fn get_gearhash_table(&self) -> &crypto::GearHashTable {
        &self.gearhash_table
    }

    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> anyhow::Result<Key> {
        // We have to use `remote IS ?` here because we want NULL = NULL (it is not remote).
        let query: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT key FROM reflog WHERE refname=? AND remote IS ? ORDER BY id DESC LIMIT 1",
                params![refname, remote],
                |row| row.get(0),
            )
            .optional()?;

        match query {
            Some(k) => Ok(Key::from_db_key(&k)?),
            None => anyhow::bail!("key not found"),
        }
    }

    fn reflog_push(&self, data: &Reflog) -> anyhow::Result<()> {
        self.conn
            .execute(
                "INSERT INTO reflog(refname, remote, key) VALUES (?, ?, ?)",
                params![data.refname, data.remote, data.key.as_db_key(),],
            )?;
            Ok(())
    }

    fn reflog_walk(&self, refname: &str, remote: Option<&str>) -> anyhow::Result<Vec<Key>> {
        let mut statement = self
            .conn
            .prepare("SELECT key FROM reflog WHERE refname=? AND remote IS ? ORDER BY id DESC")?;

        let mut rows = statement.query(params![refname, remote]).unwrap();

        let mut keys = Vec::new();

        while let Some(row) = rows.next().unwrap() {
            let buf: Vec<u8> = row.get(0).unwrap();
            keys.push(Key::from_db_key(&buf)?);
        }

        Ok(keys)
    }

    fn raw_get<'a>(&'a self, key: &[u8]) -> anyhow::Result<Cow<'a, [u8]>> {
        let results: Vec<u8> = self
            .conn
            .query_row("SELECT value FROM data WHERE key=?", params![key], |row| {
                row.get(0)
            })?;

        Ok(Cow::Owned(results))
    }

    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> anyhow::Result<()> {
        self.conn
            .prepare_cached("INSERT OR IGNORE INTO data VALUES (?, ?)")?
            .execute(params![key, data])?;

        Ok(())
    }

    fn raw_get_state(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let results: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT value FROM state WHERE key=?", params![key], |row| {
                row.get(0)
            })
            .optional()?;

        Ok(results)
    }

    fn raw_put_state(&self, key: &[u8], data: &[u8]) -> anyhow::Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO state VALUES (?, ?)",
                params![key, data],
            )
            ?;

        Ok(())
    }

    fn raw_exists(&self, key: &[u8]) -> anyhow::Result<bool> {
        let count: u32 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM data WHERE key=?",
                params![key],
                |row| row.get(0),
            )
            ?;

        assert!(count == 0 || count == 1);

        Ok(count == 1)
    }

    fn raw_between(&self, start: &[u8], end: Option<&[u8]>) -> anyhow::Result<Vec<Vec<u8>>> {
        let mut results = Vec::new();
        if let Some(e) = end {
            let mut statement = self
                .conn
                .prepare("SELECT key FROM data WHERE key >= ? AND key < ?")
                ?;

            let rows = statement
                .query_map(params![start, e], |row| row.get(0))
                ?;

            for row in rows {
                results.push(row?);
            }
        } else {
            let mut statement = self
                .conn
                .prepare("SELECT key FROM data WHERE key >= ?") ?;
            let rows = statement
                .query_map(params![start], |row| row.get(0)) ?;

            for row in rows {
                results.push(row?);
            }
        }

        tracing::trace!("... got results {:?}", &results);
        Ok(results)
    }
}
