use crate::crypto;
use crate::ds;
use crate::ds::{
    GetReflogError, RawBetweenError, RawExistsError, RawGetError, RawGetStateError, RawPutError,
    RawPutStateError, ReflogPushError, WalkReflogError,
};
use crate::key::Key;
use crate::Reflog;
use std::borrow::Cow;

#[derive(Debug)]
pub struct NullDs(crypto::RepoKey);

impl NullDs {
    pub fn new() -> Self {
        NullDs(crypto::RepoKey::zero_key())
    }
}

impl Default for NullDs {
    fn default() -> Self {
        Self::new()
    }
}

impl ds::Transactional for NullDs {}

impl crate::DataStore for NullDs {
    fn get_repokey(&self) -> &crate::crypto::RepoKey {
        &self.0
    }

    fn raw_get<'a>(&'a self, _key: &[u8]) -> Result<Cow<'a, [u8]>, RawGetError> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put<'a>(&'a self, _key: &[u8], _data: &[u8]) -> Result<(), RawPutError> {
        Ok(())
    }
    fn raw_exists(&self, _key: &[u8]) -> Result<bool, RawExistsError> {
        unimplemented!("null datastore, no data")
    }
    fn raw_get_state(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, RawGetStateError> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put_state(&self, _key: &[u8], _data: &[u8]) -> Result<(), RawPutStateError> {
        Ok(())
    }
    fn reflog_push(&self, _data: &Reflog) -> Result<(), ReflogPushError> {
        Ok(())
    }
    fn reflog_get(&self, _refname: &str, _remote: Option<&str>) -> Result<Key, GetReflogError> {
        unimplemented!("null datastore, no data")
    }
    fn reflog_walk(
        &self,
        _refname: &str,
        _remote: Option<&str>,
    ) -> Result<Vec<Key>, WalkReflogError> {
        unimplemented!("null datastore, no data")
    }

    fn raw_between(
        &self,
        _start: &[u8],
        _end: Option<&[u8]>,
    ) -> Result<Vec<Vec<u8>>, RawBetweenError> {
        unimplemented!("null datastore, no data")
    }
}
