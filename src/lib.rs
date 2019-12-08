use blake2::{Blake2b, Digest};

use rusqlite::params;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Cursor;

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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(hex::decode(s).unwrap()))
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

pub trait DataStore {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]>;
    fn put(&mut self, data: Vec<u8>) -> KeyBuf;

    fn get_obj(&self, key: Key) -> Object {
        let data = self.get(key);

        serde_cbor::from_slice(&data).unwrap()
    }

    fn put_obj(&mut self, data: &Object) -> KeyBuf {
        let data = serde_cbor::to_vec(data).unwrap();

        self.put(data)
    }
}

pub struct SqliteDS {
    conn: rusqlite::Connection,
}

impl SqliteDS {
    #[must_use]
    pub fn new(path: &str) -> Self {
        let conn = rusqlite::Connection::open(path).unwrap();

        conn.pragma_update(None, &"journal_mode", &"WAL").unwrap();

        conn.execute(
            "
            CREATE TABLE IF NOT EXISTS data (
                key BLOB NOT NULL UNIQUE PRIMARY KEY,
                value BLOB NOT NULL
            ) WITHOUT ROWID
        ",
            params![],
        )
        .unwrap();

        Self { conn }
    }
}

impl DataStore for SqliteDS {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]> {
        let results: Vec<u8> = self
            .conn
            .query_row(
                "SELECT value FROM data WHERE key=?",
                params![key.0],
                |row| row.get(0),
            )
            .unwrap();

        let cursor = Cursor::new(results);

        let decompressed = zstd::decode_all(cursor).unwrap();

        Cow::Owned(decompressed)
    }

    fn put(&mut self, data: Vec<u8>) -> KeyBuf {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();

        let cursor = Cursor::new(data);
        let compressed = zstd::encode_all(cursor, 6).unwrap();

        self.conn
            .execute(
                "INSERT OR IGNORE INTO data VALUES (?, ?)",
                params![hash, compressed],
            )
            .unwrap();

        KeyBuf(hash)
    }
}

#[derive(Debug, Default)]
pub struct HashSetDS {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl DataStore for HashSetDS {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]> {
        Cow::Borrowed(&self.data[&*key.0])
    }

    fn put(&mut self, data: Vec<u8>) -> KeyBuf {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        self.data.insert(hash.clone(), data);
        KeyBuf(hash)
    }
}

#[derive(Debug, Default)]
pub struct NullB2DS {}

impl DataStore for NullB2DS {
    fn get<'a>(&'a self, _key: Key) -> Cow<'a, [u8]> {
        Cow::Borrowed(&[0; 0])
    }

    fn put(&mut self, data: Vec<u8>) -> KeyBuf {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        KeyBuf(hash)
    }
}
