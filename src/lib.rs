use std::path::Path;
use failure_derive::Fail;
use blake2::{Blake2b, Digest};
use bitvec::prelude::*;
use proptest::prelude::*;

use rusqlite::params;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Cursor;

use failure::{Fallible, bail};

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
        fmt.write_str(&to_base32(self.0.clone()))
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

#[derive(Debug)]
pub enum Keyish {
    /// Strictly speaking, this is for prefix searches
    ///
    /// .0 will be a value for which all keys that match the prefix will be lexographically ordered
    /// afterwards. For display, an encoded form of .0 should be used.
    Range(String, Vec<u8>, Option<Vec<u8>>)
}

#[derive(Debug, Fail)]
pub enum KeyishParseError {
    #[fail(display = "{} is an invalid key", _0)]
    Invalid(String)
}

impl std::str::FromStr for Keyish {
    type Err = KeyishParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = match from_base32(s) {
            Ok(v) => v,
            Err(_) => Err(KeyishParseError::Invalid(s.to_string()))?,
        };

        let start = input.clone();

        let mut end = input.clone();

        end += bitvec![BigEndian, u8; 1];

        let did_overflow = start.all();

        let ret_start = start.into_vec();

        let ret_end = if did_overflow {
            None
        } else {
            Some(end.into_vec())
        };

        Ok(Keyish::Range(s.to_string(), ret_start, ret_end))
    }
}

fn u5_to_bitvec(x: u8) -> BitVec<BigEndian, u8> {
    bitvec![BigEndian, u8; x & 0b10000,
     x & 0b01000,
     x & 0b00100,
     x & 0b00010,
     x & 0b00001]
}

fn pop_u5_from_bitvec(x: &mut BitVec<BigEndian, u8>) -> u8 {
    let mut v = 0;
    for _ in 0..5 {
        if let Some(bit) = x.pop() {
            v<<=1; v |= bit as u8;
        } else {
            return v;
        }
    }
    v
}

fn from_base32(x: &str) -> Fallible<BitVec<BigEndian, u8>> {
    let mut result = BitVec::<BigEndian, u8>::new();

    for mut ch in x.bytes() {
        if (b'A'..=b'Z').contains(&ch) {
            ch |= 0b00100000; // Convert to uppercase
        }

        let idx = table.iter().position(|&x| x == ch).unwrap();

        result.extend(u5_to_bitvec(idx as u8));
        
        /*
        match ch {
            0_u8..=b'1' => bail!("invalid input"),
            b'2'..=b'7' => result.extend(u5_to_bitvec((ch - b'2') + 26)),
            b'8'..=b'@' => bail!("invalid input"),
            b'A'..=b'Z' => result.extend(u5_to_bitvec(ch - b'A')),
            _ => bail!("invalid input"),
        }*/
    }

    Ok(result)
}

static table: [u8; 32] = *b"abcdefghijklmnopqrstuvwxyz234567";

fn to_base32(x: Vec<u8>) -> String {
    let mut scratch = BitVec::<BigEndian, u8>::from_vec(x);
    let mut ret = String::new();

    while !scratch.is_empty() {
        let v = pop_u5_from_bitvec(&mut scratch);
        ret.push(table[v as usize] as char);
    }

    ret
}

proptest! {
    #[test]
    fn test_base_conv(x: Vec<u8>) {
        let s = to_base32(x.clone());
        dbg!(&s);
        let parsed = from_base32(&s).unwrap().into_vec();
        prop_assert_eq!(parsed, x);
    }
}

#[derive(Debug, Fail)]
pub enum CanonicalizeError {
    #[fail(display = "Invalid object id {}", _0)]
    InvalidHex(String),

    #[fail(display = "Object id {} not found", _0)]
    NotFound(String),

    #[fail(display = "Object id {} is ambiguous", _0)]
    Ambigious(String, Vec<KeyBuf>),
}

pub trait DataStore {
    fn get<'a>(&'a self, key: Key) -> Fallible<Cow<'a, [u8]>>;
    fn put(&mut self, data: Vec<u8>) -> Fallible<KeyBuf>;

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError>;

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
    pub fn new<S: AsRef<Path>>(path: S) -> Fallible<Self> {
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

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError> {
        dbg!(&search);

        match search {
            Keyish::Range(s, start, end) => {
                let mut results: Vec<Vec<u8>>;

                if let Some(e) = end {
                    let mut statement = self.conn.prepare("SELECT key FROM data WHERE key >= ? AND key < ?").unwrap();
                    let rows = statement.query_map(params![start, e], |row| row.get(0)).unwrap();

                    results = Vec::new();

                    for row in rows {
                        results.push(row.unwrap());
                    }
                } else {
                    let mut statement = self.conn.prepare("SELECT key FROM data WHERE key >= ?").unwrap();
                    let rows = statement.query_map(params![start], |row| row.get(0)).unwrap();

                    results = Vec::new();

                    for row in rows {
                        results.push(row.unwrap());
                    }
                }

                match results.len() {
                    0 => Err(CanonicalizeError::NotFound(s)),
                    1 => Ok(KeyBuf(results.pop().unwrap())),
                    _ => {
                        let strs = results.into_iter().map(|x| KeyBuf(x)).collect();
                        Err(CanonicalizeError::Ambigious(s, strs))
                    }
                }
            }
        }
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

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError> {
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

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError> {
        unimplemented!();
    }
}
