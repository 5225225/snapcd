pub mod null;
//pub mod sled;
pub mod sqlite;
//pub mod rocks;

use blake3::hash;

use std::borrow::Cow;

use thiserror::Error;

use crate::commit;
use crate::key;
use crate::key::Key;
use crate::key::TypedKey;
use crate::Keyish;
use crate::Object;

#[derive(Debug, Error)]
pub enum CanonicalizeError {
    #[error("Invalid object id '{_0}'")]
    InvalidHex(String),

    #[error("Object '{_0}' not found")]
    NotFound(String),

    #[error("Object '{_0}' is ambiguous")]
    Ambigious(String, Vec<key::Key>),

    #[error("error when converting db key: {_0}")]
    FromDbKeyError(#[from] key::FromDbKeyError),

    #[error("error when getting reflog: {_0}")]
    GetReflogError(#[from] GetReflogError),
}

pub struct Reflog {
    pub refname: String,
    pub key: TypedKey<commit::Commit>,
    pub remote: Option<String>,
}

#[derive(Debug, Error)]
pub enum GetReflogError {
    #[error("Ref not found")]
    NotFound,

    #[error("error parsing db key: {_0}")]
    FromDbKeyError(#[from] key::FromDbKeyError),

    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum DSError {
    #[error("sqlite error: {_0}")]
    SqliteError(#[from] rusqlite::Error),
}

pub trait ToDSError {
    fn to_ds(self) -> DSError;
}

pub trait ToDSErrorResult<T> {
    fn to_ds_r(self) -> Result<T, DSError>;
}

impl<T: Into<DSError>> ToDSError for T {
    fn to_ds(self) -> DSError {
        self.into()
    }
}

impl<T, E: ToDSError> ToDSErrorResult<T> for Result<T, E> {
    fn to_ds_r(self) -> Result<T, DSError> {
        self.map_err(|x| x.to_ds())
    }
}

#[derive(Debug, Error)]
pub enum BeginTransError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum RollbackTransError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum CommitTransError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum RawGetError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum RawPutError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum RawExistsError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum RawGetStateError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum RawPutStateError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum ReflogPushError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}
#[derive(Debug, Error)]
pub enum RawBetweenError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}
#[derive(Debug, Error)]
pub enum RawGetHeadError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}
#[derive(Debug, Error)]
pub enum RawPutHeadError {
    #[error(transparent)]
    DSerror(#[from] DSError),
}

#[derive(Debug, Error)]
pub enum GetHeadError {
    #[error("error when getting state: {_0}")]
    RawGetStateError(#[from] RawGetStateError),

    #[error("error decoding utf8 string: {_0}")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, Error)]
pub enum GetObjError {
    #[error("error getting object: {_0}")]
    RawGetError(#[from] RawGetError),

    #[error("error decoding object: {_0}")]
    DecodeError(#[from] serde_cbor::error::Error),
}

#[derive(Debug, Error)]
pub enum PutObjError {
    #[error("error putting object: {_0}")]
    RawPutError(#[from] RawPutError),

    #[error("error encoding object: {_0}")]
    EncodeError(#[from] serde_cbor::error::Error),
}

pub trait Transactional {
    fn begin_trans(&mut self) -> Result<(), BeginTransError> {
        Ok(())
    }
    fn commit(&mut self) -> Result<(), CommitTransError> {
        Ok(())
    }
    fn rollback(&mut self) -> Result<(), RollbackTransError> {
        Ok(())
    }
}

static_assertions::assert_obj_safe!(DataStore);
pub trait DataStore: Transactional {
    fn raw_get<'a>(&'a self, key: &[u8]) -> Result<Cow<'a, [u8]>, RawGetError>;
    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Result<(), RawPutError>;

    fn raw_exists(&self, key: &[u8]) -> Result<bool, RawExistsError>;

    fn raw_get_state<'a>(&'a self, key: &[u8]) -> Result<Option<Vec<u8>>, RawGetStateError>;
    fn raw_put_state<'a>(&'a self, key: &[u8], data: &[u8]) -> Result<(), RawPutStateError>;

    fn get(&self, key: key::Key) -> Result<Cow<'_, [u8]>, RawGetError> {
        let results = self.raw_get(&key.as_db_key())?;

        Ok(results)
    }

    fn hash(&self, data: &[u8]) -> key::Key {
        let b3 = hash(data);
        key::Key::Blake3B(*b3.as_bytes())
    }

    fn put(&self, data: Vec<u8>) -> Result<key::Key, RawPutError> {
        let keybuf = self.hash(&data);

        self.raw_put(&keybuf.as_db_key(), &data)?;

        Ok(keybuf)
    }

    fn get_head(&self) -> Result<Option<String>, GetHeadError> {
        let bytes = self.raw_get_state(b"HEAD")?;

        Ok(match bytes {
            Some(b) => Some(String::from_utf8(b)?),
            None => None,
        })
    }

    fn put_head(&self, head: &str) -> Result<(), RawPutStateError> {
        self.raw_put_state(b"HEAD", head.as_bytes())?;
        Ok(())
    }

    fn reflog_push(&self, data: &Reflog) -> Result<(), ReflogPushError>;
    fn reflog_get(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<TypedKey<commit::Commit>, GetReflogError>;
    fn reflog_walk(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<Vec<TypedKey<commit::Commit>>, WalkReflogError>;

    fn raw_between(
        &self,
        start: &[u8],
        end: Option<&[u8]>,
    ) -> Result<Vec<Vec<u8>>, RawBetweenError>;

    fn canonicalize(&self, search: Keyish) -> Result<key::Key, CanonicalizeError> {
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
                Ok(key) => return Ok(key.inner()),
                Err(GetReflogError::NotFound) => return Err(CanonicalizeError::NotFound(orig)),
                Err(e) => return Err(e.into()),
            },
        };

        match results.len() {
            0 => Err(CanonicalizeError::NotFound(err_str)),
            // This is okay since we know it will have one item.
            #[allow(clippy::option_unwrap_used)]
            1 => Ok(key::Key::from_db_key(&results.pop().unwrap())?),
            _ => {
                let strs: Result<_, _> = results
                    .into_iter()
                    .map(|x| key::Key::from_db_key(&x))
                    .collect();
                Err(CanonicalizeError::Ambigious(err_str, strs?))
            }
        }
    }

    fn get_obj(&self, key: Key) -> Result<Object, GetObjError> {
        let data = self.get(key)?;

        Ok(serde_cbor::from_slice(&data)?)
    }

    fn put_obj(&self, data: &Object) -> Result<Key, PutObjError> {
        let data = serde_cbor::to_vec(data)?;

        Ok(self.put(data)?)
    }
}

#[derive(Debug, Error)]
pub enum WalkReflogError {
    #[error("error parsing db key: {_0}")]
    FromDbKeyError(#[from] key::FromDbKeyError),

    #[error(transparent)]
    DSerror(#[from] DSError),
}
