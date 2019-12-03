#![deny(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]

use std::io::Cursor;
use std::borrow::Cow;
use std::io::prelude::*;
use blake2::{Blake2b, Digest};
use std::collections::HashMap;
use std::mem;
use cdc::RollingHash64;
use rusqlite::params;

#[derive(Debug, Clone, Copy)]
pub struct Key<'a>(&'a [u8]);

#[derive(Debug, Default, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct KeyBuf(Vec<u8>);

impl KeyBuf {
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

#[derive(Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Object<'a> {
    Blob(Cow<'a, [u8]>),
    Keys(Cow<'a, [KeyBuf]>),
}

const DISPLAY_CHUNK_SIZE: usize = 20;
impl<'a> std::fmt::Display for Object<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Object::Blob(blob) => {
                writeln!(fmt, "blob")?;
                for chunk in blob.chunks(DISPLAY_CHUNK_SIZE) {
                    let ashex = hex::encode(chunk);
                    writeln!(fmt, "{}", ashex)?;
                }

                Ok(())
            }
            Object::Keys(keys) => {
                writeln!(fmt, "keys")?;

                for key in keys.iter() {
                    writeln!(fmt, "{}", key)?;
                }

                Ok(())
            }
        }
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

    fn put_data<R: Read>(&mut self, data: R) -> KeyBuf {
        let mut key_bufs: [Vec<KeyBuf>; 5] = Default::default();

        let mut current_chunk = Vec::new();

        let mut hasher = cdc::Rabin64::new(6);

        for byte_r in data.bytes() {
            let byte = byte_r.unwrap();

            current_chunk.push(byte);
            hasher.slide(&byte);

            if current_chunk.len() < 1<<BLOB_ZERO_COUNT_MIN {
                continue;
            }

            let h = !hasher.get_hash();

            let zeros = h.trailing_zeros();

            if zeros > BLOB_ZERO_COUNT || current_chunk.len() >= 1<<(BLOB_ZERO_COUNT_MAX) {
                hasher.reset();

                let key = self.put_obj(&Object::Blob(Cow::Borrowed(&current_chunk)));
                key_bufs[0].push(key);
                current_chunk.clear();

                for offset in 0..4 {
                    let len = key_bufs[offset as usize].len();
                    if zeros > BLOB_ZERO_COUNT + (offset + 1) * PER_LEVEL_COUNT || len >= 1<<PER_LEVEL_COUNT_MAX { 
                        let key = self.put_obj(&Object::Keys(Cow::Borrowed(&key_bufs[offset as usize])));
                        key_bufs[offset as usize].clear();
                        key_bufs[offset as usize + 1].push(key);
                    } else {
                        break;
                    }
                }
            }

        }

        println!("#{} {:?}", current_chunk.len(), &key_bufs.iter().map(Vec::len).collect::<Vec<_>>());
        if !current_chunk.is_empty() {
            let data = mem::replace(&mut current_chunk, Vec::new());
            let key = self.put_obj(&Object::Blob(Cow::Borrowed(&data)));
            key_bufs[0].push(key);
        }

        for offset in 0..4 {
            println!("!{} {} {:?}", offset, current_chunk.len(), &key_bufs.iter().map(Vec::len).collect::<Vec<_>>());
            let keys = mem::replace(&mut key_bufs[offset], Vec::new());
            let key = self.put_obj(&Object::Keys(Cow::Borrowed(&keys)));
            key_bufs[offset + 1].push(key);
        }
        println!("^{} {:?}", current_chunk.len(), &key_bufs.iter().map(Vec::len).collect::<Vec<_>>());

        assert!(key_bufs[0].is_empty());
        assert!(key_bufs[1].is_empty());
        assert!(key_bufs[2].is_empty());
        assert!(key_bufs[3].is_empty());

        let taken = mem::replace(&mut key_bufs[4], Vec::new());

        self.put_obj(&Object::Keys(Cow::Borrowed(&taken)))
    }

    fn read_data<W: Write>(&self, key: Key, to: &mut W) {
        let obj = self.get_obj(key);

        match obj { 
            Object::Keys(keys) => {
                for key in keys.iter() {
                    self.read_data(key.as_key(), to);
                }
            }
            Object::Blob(vec) => {
                to.write_all(&vec).expect("failed to write to out");
            }
        }
    }
}

pub struct SqliteDS {
    conn: rusqlite::Connection,
}

impl SqliteDS {
    pub fn new(path: &str) -> Self {
        let conn = rusqlite::Connection::open(path).unwrap();

        conn.pragma_update(None, &"journal_mode", &"WAL").unwrap();

        conn.execute("
            CREATE TABLE IF NOT EXISTS data (
                key BLOB NOT NULL UNIQUE PRIMARY KEY,
                value BLOB NOT NULL
            ) WITHOUT ROWID
        ", params![]).unwrap();

        Self {
            conn
        }
    }
}

impl DataStore for SqliteDS {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]> {
        let results: Vec<u8> = self.conn.query_row(
            "SELECT value FROM data WHERE key=?",
            params![key.0],
            |row| row.get(0)).unwrap();

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

        self.conn.execute(
            "INSERT OR IGNORE INTO data VALUES (?, ?)",
            params![hash, compressed]).unwrap();

        KeyBuf(hash)
    }
}

const BLOB_ZERO_COUNT_MIN: u32 = BLOB_ZERO_COUNT - 2;
const BLOB_ZERO_COUNT: u32 = 12;
const BLOB_ZERO_COUNT_MAX: u32 = BLOB_ZERO_COUNT + 2;

const PER_LEVEL_COUNT: u32 = 5;
const PER_LEVEL_COUNT_MAX: u32 = PER_LEVEL_COUNT + 2;

pub fn put_data<DS: DataStore, R: Read>(data: R, store: &mut DS) -> KeyBuf {
    store.put_data(data)
}

pub fn read_data<DS: DataStore, W: Write>(key: Key, store: &DS, to: &mut W) {
    store.read_data(key, to);
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
pub struct NullB2DS {
}

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
