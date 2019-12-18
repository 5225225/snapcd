use bitvec::prelude::*;
use blake2::{VarBlake2b, digest::Input};
use blake2::digest::VariableOutput;
use failure_derive::Fail;
use std::path::Path;

use rusqlite::params;
use std::borrow::Cow;
use std::io::Cursor;

use failure::Fallible;

pub mod dir;
pub mod file;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum KeyBuf {
    Blake2B(Vec<u8>),
}

impl KeyBuf {
    fn hash_id(&self) -> u8 {
        match self {
            Self::Blake2B(_) => 1,
        }
    }

    fn hash_bytes(&self) -> &[u8] {
        match self {
            Self::Blake2B(x) => &x,
        }
    }

    fn from_db_key(x: &[u8]) -> Self {
        let hash_id = x[0];
        let hash_bytes = &x[1..];

        match hash_id {
            1 => Self::Blake2B(hash_bytes.to_vec()),
            0 | 2..=255 => panic!("invalid key"),
        }
    }


    fn as_db_key(&self) -> Vec<u8> {
        let hash_id = self.hash_id();
        let hash_bytes = self.hash_bytes();

        let mut result = Vec::with_capacity(hash_bytes.len() + 1);

        result.push(hash_id);
        result.extend(hash_bytes);

        result
    }

    fn as_user_key(&self) -> String {
        let mut result = String::new();

        let prefix = match self {
            Self::Blake2B(_) => "b"
        };

        result.push_str(prefix);

        let encoded = to_base32(self.hash_bytes());

        result.push_str(&encoded);

        result
    }

}

impl std::fmt::Display for KeyBuf {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        fmt.write_str(&self.as_user_key())
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
    Range(String, Vec<u8>, Option<Vec<u8>>),
}

#[derive(Debug, Fail)]
pub enum KeyishParseError {
    #[fail(display = "{} is an invalid key", _0)]
    Invalid(String),
}

impl std::str::FromStr for Keyish {
    type Err = KeyishParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (prefix, bytes) = (&s[0..1], &s[1..]);

        let max_len = match prefix {
            "b" => 64*8,
            _ => return Err(KeyishParseError::Invalid(s.to_string())),
        };

        let input = match from_base32(bytes, max_len) {
            Ok(v) => v,
            Err(_) => return Err(KeyishParseError::Invalid(s.to_string())),
        };

        let did_overflow = input.all();

        let start = input.clone();

        let mut ret_start = start.into_vec();

        ret_start.insert(0_usize, 1);

        let ret_end = if did_overflow {
            None
        } else {
            let mut end = input;

            end += bitvec![BigEndian, u8; 1];

            let mut v = end.into_vec();

            v.insert(0_usize, 1);

            Some(v)
        };

        Ok(dbg!(Keyish::Range(s.to_string(), ret_start, ret_end)))
    }
}

fn pop_u5_from_bitvec(x: &mut BitVec<BigEndian, u8>) -> u8 {
    let mut v = 0;
    for to_shift in (0..5).rev() {
        if x.is_empty() {
            return v << to_shift;
        }

        let bit = x.remove(0);
        v <<= 1;
        v |= bit as u8;
    }

    assert!(v <= 31);

    v
}

#[derive(Debug, Fail)]
pub enum FromBase32Error {
    #[fail(display = "found non-base32 char {}", _0)]
    UnknownByte(char),
}

fn from_base32(x: &str, max_len: usize) -> Fallible<BitVec<BigEndian, u8>> {
    let mut result = BitVec::<BigEndian, u8>::new();

    for mut ch in x.bytes() {
        if (b'A'..=b'Z').contains(&ch) {
            ch |= 0b0010_0000; // Convert to uppercase
        }

        let idx = TABLE
            .iter()
            .position(|&x| x == ch)
            .ok_or_else(|| FromBase32Error::UnknownByte(ch as char))?;

        result.push(idx & 0b10000 != 0);
        result.push(idx & 0b01000 != 0);
        result.push(idx & 0b00100 != 0);
        result.push(idx & 0b00010 != 0);
        result.push(idx & 0b00001 != 0);
    }

    result.truncate(max_len);

    Ok(result)
}

static TABLE: [u8; 32] = *b"abcdefghijklmnopqrstuvwxyz234567";

fn to_base32(x: &[u8]) -> String {
    let mut scratch = BitVec::<BigEndian, u8>::from_vec(x.to_vec());
    let mut ret = String::new();
    while !scratch.is_empty() {
        let v = pop_u5_from_bitvec(&mut scratch);
        ret.push(TABLE[v as usize] as char);
    }

    ret
}

#[derive(Debug, Fail)]
pub enum CanonicalizeError {
    #[fail(display = "Invalid object id {}", _0)]
    InvalidHex(String),

    #[fail(display = "Object id {} not found", _0)]
    NotFound(String),

    #[fail(display = "Object id {} is ambiguous", _0)]
    Ambigious(String, Vec<KeyBuf>),

    #[fail(display = "{}", _0)]
    Unknown(failure::Error),
}

impl<T: std::error::Error + Send + Sync + 'static> std::convert::From<T> for CanonicalizeError {
    fn from(err: T) -> Self {
        Self::Unknown(failure::Error::from_boxed_compat(Box::new(err)))
    }
}

pub trait DataStore {
    fn get<'a>(&'a self, key: &KeyBuf) -> Fallible<Cow<'a, [u8]>>;
    fn put(&self, data: Vec<u8>) -> Fallible<KeyBuf>;

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError>;

    fn get_obj(&self, key: &KeyBuf) -> Fallible<Object> {
        let data = self.get(key)?;

        Ok(serde_cbor::from_slice(&data)?)
    }

    fn put_obj(&self, data: &Object) -> Fallible<KeyBuf> {
        let data = serde_cbor::to_vec(data)?;

        Ok(self.put(data)?)
    }

    fn begin_trans(&mut self) {}
    fn commit(&mut self) {}
    fn rollback(&mut self) {}
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
    fn begin_trans(&mut self) {
        self.conn.execute("BEGIN TRANSACTION", params![]).unwrap();
    }

    fn commit(&mut self) {
        self.conn.execute("COMMIT", params![]).unwrap();
    }

    fn rollback(&mut self) {
        self.conn.execute("ROLLBACK", params![]).unwrap();
    }

    fn get<'a>(&'a self, key: &KeyBuf) -> Fallible<Cow<'a, [u8]>> {
        dbg!(key.as_db_key());
        let results: Vec<u8> = self.conn.query_row(
            "SELECT value FROM data WHERE key=?",
            params![key.as_db_key()],
            |row| row.get(0),
        )?;

        let cursor = Cursor::new(results);

        let decompressed = zstd::decode_all(cursor)?;

        Ok(Cow::Owned(decompressed))
    }

    fn put(&self, data: Vec<u8>) -> Fallible<KeyBuf> {
        let mut b2 = VarBlake2b::new(64).unwrap();
        b2.input(&data);
        let hash = b2.vec_result();

        let keybuf = KeyBuf::Blake2B(hash);

        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM data WHERE key=?",
            params![keybuf.as_db_key()],
            |row| row.get(0),
        )?;

        match count {
            0 => {
                let cursor = Cursor::new(data);

                let compressed = zstd::encode_all(cursor, 6)?;

                self.conn.execute(
                    "INSERT OR IGNORE INTO data VALUES (?, ?)",
                    params![keybuf.as_db_key(), compressed],
                )?;
            }
            1 => {}
            2..=0xffffffff => {
                failure::bail!("data error: multiple keys found for same value?");
            }
        }

        Ok(keybuf)
    }

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError> {
        match search {
            Keyish::Range(s, start, end) => {
                let mut results: Vec<Vec<u8>>;

                if let Some(e) = end {
                    let mut statement = self
                        .conn
                        .prepare("SELECT key FROM data WHERE key >= ? AND key < ?")?;

                    dbg!(&start, &e);
                    let rows = statement.query_map(params![start, e], |row| row.get(0))?;

                    results = Vec::new();

                    for row in rows {
                        results.push(row?);
                    }
                } else {
                    let mut statement = self.conn.prepare("SELECT key FROM data WHERE key >= ?")?;
                    let rows = statement.query_map(params![start], |row| row.get(0))?;

                    results = Vec::new();

                    for row in rows {
                        results.push(row?);
                    }
                }

                match results.len() {
                    0 => Err(CanonicalizeError::NotFound(s)),
                    // This is okay since we know it will have one item.
                    #[allow(clippy::option_unwrap_used)]
                    1 => Ok(KeyBuf::from_db_key(&results.pop().unwrap())),
                    _ => {
                        let strs = results.into_iter().map(KeyBuf::Blake2B).collect();
                        Err(CanonicalizeError::Ambigious(s, strs))
                    }
                }
            }
        }
    }
}
