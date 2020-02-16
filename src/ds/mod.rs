pub mod null;
pub mod sled;
pub mod sqlite;

use blake3::hash;
use failure_derive::Fail;

use std::borrow::Cow;

use failure::Fallible;

use crate::Keyish;
use crate::Object;
use crate::KeyBuf;

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
