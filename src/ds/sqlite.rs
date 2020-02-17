use std::path::Path;

use rusqlite::params;
use rusqlite::OptionalExtension;
use std::borrow::Cow;

use crate::{DataStore, GetReflogError, KeyBuf, Reflog, WalkReflogError};
use failure::Fallible;

pub struct SqliteDS {
    conn: rusqlite::Connection,
}

impl SqliteDS {
    pub fn new<S: AsRef<Path>>(path: S) -> Fallible<Self> {
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

impl DataStore for SqliteDS {
    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> Result<KeyBuf, GetReflogError> {
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

        let row = query.map_err(GetReflogError::SqliteError)?;

        let key = row.ok_or(GetReflogError::NotFound)?;

        Ok(KeyBuf::from_db_key(&key))
    }

    fn reflog_push(&self, data: &Reflog) -> Fallible<()> {
        self.conn.execute(
            "INSERT INTO reflog(refname, remote, key) VALUES (?, ?, ?)",
            params![data.refname, data.remote, data.key.as_db_key(),],
        )?;

        Ok(())
    }

    fn reflog_walk(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<Vec<KeyBuf>, WalkReflogError> {
        let mut statement = self
            .conn
            .prepare("SELECT key FROM reflog WHERE refname=? AND remote IS ? ORDER BY id DESC")
            .unwrap();

        let mut rows = statement.query(params![refname, remote]).unwrap();

        let mut keys = Vec::new();

        while let Some(row) = rows.next().unwrap() {
            let buf: Vec<u8> = row.get(0).unwrap();
            keys.push(KeyBuf::from_db_key(&buf));
        }

        Ok(keys)
    }

    fn begin_trans(&mut self) -> Fallible<()> {
        self.conn.execute("BEGIN TRANSACTION", params![])?;
        Ok(())
    }

    fn commit(&mut self) -> Fallible<()> {
        self.conn.execute("COMMIT", params![])?;
        Ok(())
    }

    fn rollback(&mut self) -> Fallible<()> {
        self.conn.execute("ROLLBACK", params![])?;
        Ok(())
    }

    fn raw_get<'a>(&'a self, key: &[u8]) -> Fallible<Cow<'a, [u8]>> {
        let results: Vec<u8> =
            self.conn
                .query_row("SELECT value FROM data WHERE key=?", params![key], |row| {
                    row.get(0)
                })?;

        Ok(Cow::Owned(results))
    }

    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
        self.conn
            .prepare_cached("INSERT OR IGNORE INTO data VALUES (?, ?)")?
            .execute(params![key, data])?;

        Ok(())
    }

    fn raw_get_state<'a>(&'a self, key: &[u8]) -> Fallible<Option<Vec<u8>>> {
        let results: Result<Option<Vec<u8>>, _> = self
            .conn
            .query_row("SELECT value FROM state WHERE key=?", params![key], |row| {
                row.get(0)
            })
            .optional();

        Ok(results?)
    }

    fn raw_put_state<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO state VALUES (?, ?)",
            params![key, data],
        )?;

        Ok(())
    }

    fn raw_exists(&self, key: &[u8]) -> Fallible<bool> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM data WHERE key=?",
            params![key],
            |row| row.get(0),
        )?;

        assert!(count == 0 || count == 1);

        Ok(count == 1)
    }

    fn raw_between(&self, start: &[u8], end: Option<&[u8]>) -> Fallible<Vec<Vec<u8>>> {
        dbg!(&start, &end);
        let mut results = Vec::new();
        if let Some(e) = end {
            let mut statement = self
                .conn
                .prepare("SELECT key FROM data WHERE key >= ? AND key < ?")?;

            let rows = statement.query_map(params![start, e], |row| row.get(0))?;

            for row in rows {
                results.push(row?);
            }
        } else {
            let mut statement = self.conn.prepare("SELECT key FROM data WHERE key >= ?")?;
            let rows = statement.query_map(params![start], |row| row.get(0))?;

            for row in rows {
                results.push(row?);
            }
        }

        Ok(results)
    }
}
