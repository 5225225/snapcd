use bitvec::prelude::*;
use blake2::{digest::Digest, Blake2b};
use failure_derive::Fail;
use std::path::Path;

use rusqlite::params;
use rusqlite::OptionalExtension;
use std::borrow::Cow;
use std::io::Cursor;

use failure::Fallible;

pub mod cache;
pub mod commit;
pub mod diff;
pub mod dir;
pub mod file;
pub mod filter;

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
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

    pub fn as_user_key(&self) -> String {
        let mut result = String::new();

        let prefix = match self {
            Self::Blake2B(_) => "b",
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

    pub fn debug_pretty_print(&self) -> impl std::fmt::Display + '_ {
        ObjectPrettyPrinter(self)
    }

    pub fn show(&self) -> impl std::fmt::Display + '_ {
        ObjectShowPrinter(self)
    }
}

struct ObjectPrettyPrinter<'a>(&'a Object<'a>);

const DISPLAY_CHUNK_SIZE: usize = 20;
impl<'a> std::fmt::Display for ObjectPrettyPrinter<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        writeln!(fmt, "--type: {:?}--", self.0.objtype)?;

        writeln!(fmt, "--keys--")?;
        if !self.0.keys.is_empty() {
            for key in self.0.keys.iter() {
                writeln!(fmt, "{}", key)?;
            }
        }
        writeln!(fmt, "-/keys--")?;

        writeln!(fmt, "--data--")?;
        if !self.0.data.is_empty() {
            for chunk in self.0.data.chunks(DISPLAY_CHUNK_SIZE) {
                let ashex = hex::encode(chunk);
                writeln!(fmt, "{}", ashex)?;
            }
        }
        writeln!(fmt, "-/data--")?;

        writeln!(fmt, "--deserialised data--")?;

        match serde_cbor::from_slice::<serde_cbor::Value>(&self.0.data) {
            Ok(v) => {
                println!("{:?}", v);
            }
            Err(e) => {
                println!("error when deserialising!");
                println!("{:?}", e);
            }
        };
        writeln!(fmt, "--/deserialised data--")?;

        Ok(())
    }
}

struct ObjectShowPrinter<'a>(&'a Object<'a>);

impl<'a> std::fmt::Display for ObjectShowPrinter<'a> {
    fn fmt(&self, _fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self.0.objtype {
            _ => {
                debug_assert!(false, "unable to format {}", self.0.objtype);
                Err(std::fmt::Error)
            }
        }
    }
}

#[derive(Debug)]
pub enum Keyish {
    /// Strictly speaking, this is for prefix searches
    ///
    /// .0 will be a value for which all keys that match the prefix will be lexographically ordered
    /// afterwards. For display, an encoded form of .0 should be used.
    Range(String, Vec<u8>, Option<Vec<u8>>),

    /// An exact key.
    Key(String, Vec<u8>),

    Reflog {
        orig: String,
        remote: Option<String>,
        keyname: String,
    },
}

#[derive(Debug, Fail)]
pub enum KeyishParseError {
    #[fail(display = "{} is an invalid key", _0)]
    Invalid(String),
}

impl std::str::FromStr for Keyish {
    type Err = KeyishParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains('/') {
            return parse_from_ref(s);
        } else {
            return parse_from_base32(s);
        }

        fn parse_from_ref(s: &str) -> Result<Keyish, KeyishParseError> {
            let idx = s
                .find('/')
                .expect("should only be called if s contains a /");

            if idx == 0 {
                Ok(Keyish::Reflog {
                    orig: s.to_string(),
                    keyname: s[1..].to_string(),
                    remote: None,
                })
            } else {
                let remote = &s[0..idx];
                let keyname = &s[idx + 1..];

                Ok(Keyish::Reflog {
                    orig: s.to_string(),
                    keyname: keyname.to_string(),
                    remote: Some(remote.to_string()),
                })
            }
        }

        fn parse_from_base32(s: &str) -> Result<Keyish, KeyishParseError> {
            let (prefix, bytes) = (&s[0..1], &s[1..]);

            let max_len = match prefix {
                "b" => 64 * 8,
                _ => return Err(KeyishParseError::Invalid(s.to_string())),
            };

            let input = match from_base32(bytes, max_len) {
                Ok(v) => v,
                Err(_) => return Err(KeyishParseError::Invalid(s.to_string())),
            };

            if input.len() == max_len {
                let mut v = input.into_vec();
                v.insert(0_usize, 1);
                return Ok(Keyish::Key(s.to_string(), v));
            }

            let did_overflow = input.all();

            let start = input.clone();

            let mut ret_start = start.into_vec();

            ret_start.insert(0_usize, 1);

            let ret_end = if did_overflow {
                None
            } else {
                let mut end = input;

                end += bitvec![Msb0, u8; 1];

                let mut v = end.into_vec();

                v.insert(0_usize, 1);

                Some(v)
            };

            Ok(Keyish::Range(s.to_string(), ret_start, ret_end))
        }
    }
}

fn pop_u5_from_bitvec(x: &mut BitVec<Msb0, u8>) -> u8 {
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

fn from_base32(x: &str, max_len: usize) -> Fallible<BitVec<Msb0, u8>> {
    let mut result = BitVec::<Msb0, u8>::new();

    for mut ch in x.bytes() {
        if (b'A'..=b'Z').contains(&ch) {
            ch |= 0b0010_0000; // Convert to lowercase
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
    let mut scratch = BitVec::<Msb0, u8>::from_vec(x.to_vec());
    let mut ret = String::new();
    while !scratch.is_empty() {
        let v = pop_u5_from_bitvec(&mut scratch);
        ret.push(TABLE[v as usize] as char);
    }

    ret
}

#[derive(Debug, Fail)]
pub enum CanonicalizeError {
    #[fail(display = "Invalid object id '{}'", _0)]
    InvalidHex(String),

    #[fail(display = "Object '{}' not found", _0)]
    NotFound(String),

    #[fail(display = "Object '{}' is ambiguous", _0)]
    Ambigious(String, Vec<KeyBuf>),

    #[fail(display = "{}", _0)]
    Unknown(failure::Error),
}

impl<T: std::error::Error + Send + Sync + 'static> std::convert::From<T> for CanonicalizeError {
    fn from(err: T) -> Self {
        Self::Unknown(failure::Error::from_boxed_compat(Box::new(err)))
    }
}

pub struct Reflog {
    pub refname: String,
    pub key: KeyBuf,
    pub remote: Option<String>,
}

#[derive(Debug, Fail)]
pub enum GetReflogError {
    #[fail(display = "Ref not found")]
    NotFound,

    #[fail(display = "sqlite error: {}", _0)]
    SqliteError(rusqlite::Error),
}

pub trait DataStore {
    fn raw_get<'a>(&'a self, key: &[u8]) -> Fallible<Cow<'a, [u8]>>;
    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()>;
    fn raw_exists(&self, key: &[u8]) -> Fallible<bool>;

    fn raw_get_state<'a>(&'a self, key: &[u8]) -> Fallible<Option<Vec<u8>>>;
    fn raw_put_state<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()>;

    fn get<'a>(&'a self, key: &KeyBuf) -> Fallible<Cow<'a, [u8]>> {
        let results = self.raw_get(&key.as_db_key())?;

        let cursor = Cursor::new(results);

        let decompressed = zstd::decode_all(cursor)?;

        Ok(Cow::Owned(decompressed))
    }

    fn hash(&self, data: &[u8]) -> KeyBuf {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result();
        KeyBuf::Blake2B(hash.to_vec())
    }

    fn put(&self, data: Vec<u8>) -> Fallible<KeyBuf> {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result();

        let keybuf = KeyBuf::Blake2B(hash.to_vec());

        if !self.raw_exists(&keybuf.as_db_key())? {
            let cursor = Cursor::new(data);

            let compressed = zstd::encode_all(cursor, 6)?;

            self.raw_put(&keybuf.as_db_key(), &compressed)?;
        }

        Ok(keybuf)
    }

    fn get_head(&self) -> Fallible<Option<String>> {
        let bytes = self.raw_get_state(b"HEAD")?;

        Ok(match bytes {
            Some(b) => Some(String::from_utf8(b)?),
            None => None,
        })
    }

    fn put_head(&self, head: &str) -> Fallible<()> {
        self.raw_put_state(b"HEAD", head.as_bytes())?;
        Ok(())
    }

    fn reflog_push(&self, data: &Reflog) -> Fallible<()>;
    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> Result<KeyBuf, GetReflogError>;

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError>;

    fn get_obj(&self, key: &KeyBuf) -> Fallible<Object> {
        let data = self.get(key)?;

        Ok(serde_cbor::from_slice(&data)?)
    }

    fn put_obj(&self, data: &Object) -> Fallible<KeyBuf> {
        let data = serde_cbor::to_vec(data)?;

        Ok(self.put(data)?)
    }

    fn begin_trans(&mut self) -> Fallible<()> {
        Ok(())
    }
    fn commit(&mut self) -> Fallible<()> {
        Ok(())
    }
    fn rollback(&mut self) -> Fallible<()> {
        Ok(())
    }
}

pub struct SqliteDS {
    conn: rusqlite::Connection,
}

impl SqliteDS {
    pub fn new<S: AsRef<Path>>(path: S) -> Fallible<Self> {
        let conn = rusqlite::Connection::open(path)?;

        conn.pragma_update(None, &"journal_mode", &"WAL")?;

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
        self.conn.execute(
            "INSERT OR IGNORE INTO data VALUES (?, ?)",
            params![key, data],
        )?;

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
        self.conn
            .execute("INSERT INTO state VALUES (?, ?)", params![key, data])?;

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

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError> {
        let mut results: Vec<Vec<u8>> = Vec::new();

        let err_str;

        match search {
            Keyish::Key(s, key) => {
                err_str = s;

                let mut statement = self.conn.prepare("SELECT key FROM data WHERE key == ?")?;

                let rows = statement.query_map(params![key], |row| row.get(0))?;

                for row in rows {
                    results.push(row?);
                }
            }
            Keyish::Range(s, start, end) => {
                err_str = s;

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
            }
            Keyish::Reflog {
                orig,
                remote,
                keyname,
            } => match self.reflog_get(&keyname, remote.as_deref()) {
                Ok(key) => return Ok(key),
                Err(GetReflogError::NotFound) => return Err(CanonicalizeError::NotFound(orig)),
                Err(GetReflogError::SqliteError(e)) => return Err(e.into()),
            },
        };

        match results.len() {
            0 => Err(CanonicalizeError::NotFound(err_str)),
            // This is okay since we know it will have one item.
            #[allow(clippy::option_unwrap_used)]
            1 => Ok(KeyBuf::from_db_key(&results.pop().unwrap())),
            _ => {
                let strs = results.into_iter().map(KeyBuf::Blake2B).collect();
                Err(CanonicalizeError::Ambigious(err_str, strs))
            }
        }
    }
}
