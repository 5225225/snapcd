use failure_derive::Fail;
use blake2::{Blake2b, Digest};

use rusqlite::params;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Cursor;

use failure::Fallible;

pub mod file;
pub mod dir;

#[derive(Debug, Clone, Copy)]
pub struct Key<'a>(&'a [u8]);

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyBuf(Vec<u8>);

impl KeyBuf {
    #[must_use]
    pub fn as_key(&self) -> Key<'_> {
        Key(&self.0[..])
    }
}

impl std::fmt::Display for KeyBuf {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        fmt.write_str(&hex::encode(&self.0))
    }
}

impl std::str::FromStr for KeyBuf {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(hex::decode(s)?))
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Object<'a> {
    data: Cow<'a, [u8]>,
    keys: Cow<'a, [KeyBuf]>,
    objtype: Cow<'a, str>,
}

impl<'a> Object<'a> {
    fn only_data(data: Cow<'a, [u8]>, objtype: Cow<'a, str>) -> Self {
        Self {
            data,
            keys: Cow::Borrowed(&[]),
            objtype,
        }
    }

    fn only_keys(keys: Cow<'a, [KeyBuf]>, objtype: Cow<'a, str>) -> Self {
        Self {
            data: Cow::Borrowed(&[]),
            keys,
            objtype,
        }
    }
}

const DISPLAY_CHUNK_SIZE: usize = 20;
impl<'a> std::fmt::Display for Object<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        writeln!(fmt, "--type: {:?}--", self.objtype)?;

        writeln!(fmt, "--keys--")?;
        if !self.keys.is_empty() {
            for key in self.keys.iter() {
                writeln!(fmt, "{}", key)?;
            }
        }
        writeln!(fmt, "-/keys--")?;

        writeln!(fmt, "--data--")?;
        if !self.data.is_empty() {
            for chunk in self.data.chunks(DISPLAY_CHUNK_SIZE) {
                let ashex = hex::encode(chunk);
                writeln!(fmt, "{}", ashex)?;
            }
        }
        writeln!(fmt, "-/data--")?;

        Ok(())
    }
}

#[derive(Debug, Fail)]
pub enum CanonicalizeError {
    #[fail(display = "Invalid object id {}", _0)]
    InvalidHex(String),

    #[fail(display = "Object id {} not found", _0)]
    NotFound(String),

    #[fail(display = "Object id {} is ambiguous", _0)]
    Ambigious(String, Vec<String>),
}

pub trait DataStore {
    fn get<'a>(&'a self, key: Key) -> Fallible<Cow<'a, [u8]>>;
    fn put(&mut self, data: Vec<u8>) -> Fallible<KeyBuf>;

    fn canonicalize(&self, search: String) -> Result<KeyBuf, CanonicalizeError>;

    fn get_obj(&self, key: Key) -> Fallible<Object> {
        let data = self.get(key)?;

        Ok(serde_cbor::from_slice(&data)?)
    }

    fn put_obj(&mut self, data: &Object) -> Fallible<KeyBuf> {
        let data = serde_cbor::to_vec(data)?;

        Ok(self.put(data)?)
    }
}

pub struct SqliteDS {
    conn: rusqlite::Connection,
}

impl SqliteDS {
    pub fn new(path: &str) -> Fallible<Self> {
        let conn = rusqlite::Connection::open(path)?;

        conn.pragma_update(None, &"journal_mode", &"WAL")?;

        conn.execute(
            "
            CREATE TABLE IF NOT EXISTS data (
                key BLOB NOT NULL UNIQUE PRIMARY KEY,
                value BLOB NOT NULL
            ) WITHOUT ROWID
        ",
            params![],
        )?;

        Ok(Self { conn })
    }
}

impl DataStore for SqliteDS {
    fn get<'a>(&'a self, key: Key) -> Fallible<Cow<'a, [u8]>> {
        let results: Vec<u8> = self
            .conn
            .query_row(
                "SELECT value FROM data WHERE key=?",
                params![key.0],
                |row| row.get(0),
            )?;

        let cursor = Cursor::new(results);

        let decompressed = zstd::decode_all(cursor)?;

        Ok(Cow::Owned(decompressed))
    }

    fn put(&mut self, data: Vec<u8>) -> Fallible<KeyBuf> {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();

        let cursor = Cursor::new(data);
        let compressed = zstd::encode_all(cursor, 6)?;

        self.conn
            .execute(
                "INSERT OR IGNORE INTO data VALUES (?, ?)",
                params![hash, compressed],
            )?;

        Ok(KeyBuf(hash))
    }

    fn canonicalize(&self, search: String) -> Result<KeyBuf, CanonicalizeError> {
        unimplemented!();
    }
}

#[derive(Debug, Default)]
pub struct HashSetDS {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl DataStore for HashSetDS {
    fn get<'a>(&'a self, key: Key) -> Fallible<Cow<'a, [u8]>> {
        Ok(Cow::Borrowed(&self.data.get(&*key.0).ok_or_else(|| failure::format_err!("not found"))?))
    }

    fn put(&mut self, data: Vec<u8>) -> Fallible<KeyBuf> {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        self.data.insert(hash.clone(), data);

        Ok(KeyBuf(hash))
    }

    fn canonicalize(&self, search: String) -> Result<KeyBuf, CanonicalizeError> {
        unimplemented!();
    }
}

#[derive(Debug, Default)]
pub struct NullB2DS {}

impl DataStore for NullB2DS {
    fn get<'a>(&'a self, _key: Key) -> Fallible<Cow<'a, [u8]>> {
        Ok(Cow::Borrowed(&[0; 0]))
    }

    fn put(&mut self, data: Vec<u8>) -> Fallible<KeyBuf> {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        Ok(KeyBuf(hash))
    }

    fn canonicalize(&self, search: String) -> Result<KeyBuf, CanonicalizeError> {
        Err(CanonicalizeError::NotFound(search))
    }
}
