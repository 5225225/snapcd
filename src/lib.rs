use bitvec::prelude::*;
use blake3::hash;
use failure_derive::Fail;

use std::borrow::Cow;

use failure::Fallible;

pub mod base32;
pub mod cache;
pub mod commit;
pub mod diff;
pub mod dir;
pub mod ds;
pub mod file;
pub mod filter;

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum KeyBuf {
    Blake3B([u8; 32]),
}

impl KeyBuf {
    fn hash_id(&self) -> u8 {
        match self {
            Self::Blake3B(_) => 1,
        }
    }

    fn hash_bytes(&self) -> &[u8] {
        match self {
            Self::Blake3B(x) => x.as_ref(),
        }
    }

    pub fn from_db_key(x: &[u8]) -> Self {
        use std::convert::TryInto;

        let hash_id = x[0];
        let hash_bytes = &x[1..];

        match hash_id {
            1 => Self::Blake3B(hash_bytes.try_into().unwrap()),
            0 | 2..=255 => panic!("invalid key"),
        }
    }

    pub fn as_db_key(&self) -> Vec<u8> {
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
            Self::Blake3B(_) => "b",
        };

        result.push_str(prefix);

        let encoded = base32::to_base32(self.hash_bytes());

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
    data: Cow<'a, serde_bytes::Bytes>,
    keys: Cow<'a, [KeyBuf]>,
    objtype: Cow<'a, str>,
}

impl<'a> Object<'a> {
    fn only_data(data: Cow<'a, [u8]>, objtype: Cow<'a, str>) -> Self {
        Self {
            data: Cow::Owned(serde_bytes::ByteBuf::from(data.into_owned())),
            keys: Cow::Borrowed(&[]),
            objtype,
        }
    }

    fn only_keys(keys: Cow<'a, [KeyBuf]>, objtype: Cow<'a, str>) -> Self {
        Self {
            data: Cow::Owned(serde_bytes::ByteBuf::new()),
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

            dbg!(prefix, bytes);

            let max_len = match prefix {
                "b" => 32 * 8,
                _ => return Err(KeyishParseError::Invalid(s.to_string())),
            };

            let input = match base32::from_base32(bytes, max_len) {
                Ok(v) => v,
                Err(_) => return Err(KeyishParseError::Invalid(s.to_string())),
            };

            if input.len() == max_len {
                let mut v = input.into_vec();
                v.insert(0_usize, 1_u8);
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

static_assertions::assert_obj_safe!(DataStore);

pub trait DataStore {
    fn raw_get<'a>(&'a self, key: &[u8]) -> Fallible<Cow<'a, [u8]>>;
    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()>;

    fn raw_exists(&self, key: &[u8]) -> Fallible<bool>;

    fn raw_get_state<'a>(&'a self, key: &[u8]) -> Fallible<Option<Vec<u8>>>;
    fn raw_put_state<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()>;

    fn get<'a>(&'a self, key: &KeyBuf) -> Fallible<Cow<'a, [u8]>> {
        let results = self.raw_get(&key.as_db_key())?;

        Ok(results)
    }

    fn hash(&self, data: &[u8]) -> KeyBuf {
        let b3 = hash(data);
        KeyBuf::Blake3B(*b3.as_bytes())
    }

    fn put(&self, data: Vec<u8>) -> Fallible<KeyBuf> {
        let keybuf = self.hash(&data);

        self.raw_put(&keybuf.as_db_key(), &data)?;

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
    fn reflog_walk(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<Vec<KeyBuf>, WalkReflogError>;

    fn raw_between(&self, start: &[u8], end: Option<&[u8]>) -> Fallible<Vec<Vec<u8>>>;

    fn canonicalize(&self, search: Keyish) -> Result<KeyBuf, CanonicalizeError> {
        let mut results: Vec<Vec<u8>> = Vec::new();

        let err_str;

        match search {
            Keyish::Key(s, key) => {
                err_str = s;

                let k = self.raw_get(&key).unwrap();
                results.push(k.to_vec());
            }
            Keyish::Range(s, start, end) => {
                err_str = s;

                results = self.raw_between(&start, end.as_deref()).unwrap();
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
                let strs = results
                    .into_iter()
                    .map(|x| KeyBuf::from_db_key(&x))
                    .collect();
                Err(CanonicalizeError::Ambigious(err_str, strs))
            }
        }
    }

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

#[derive(Debug, Fail)]
pub enum WalkReflogError {
    #[fail(display = "sqlite error: {}", _0)]
    SqliteError(rusqlite::Error),
}
