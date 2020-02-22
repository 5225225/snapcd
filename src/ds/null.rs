use crate::{KeyBuf, Reflog};
use crate::ds::{GetReflogError, WalkReflogError, RawGetError, RawPutError, RawBetweenError,
RawExistsError, ReflogPushError, RawGetStateError, RawPutStateError};
use std::borrow::Cow;

pub struct NullDS;

impl crate::DataStore for NullDS {
    fn raw_get<'a>(&'a self, _key: &[u8]) -> Result<Cow<'a, [u8]>, RawGetError> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put<'a>(&'a self, _key: &[u8], _data: &[u8]) -> Result<(), RawPutError> {
        Ok(())
    }
    fn raw_exists(&self, _key: &[u8]) -> Result<bool, RawExistsError> {
        unimplemented!("null datastore, no data")
    }
    fn raw_get_state<'a>(&'a self, _key: &[u8]) -> Result<Option<Vec<u8>>, RawGetStateError> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put_state<'a>(&'a self, _key: &[u8], _data: &[u8]) -> Result<(), RawPutStateError> {
        Ok(())
    }
    fn reflog_push(&self, _data: &Reflog) -> Result<(), ReflogPushError> {
        Ok(())
    }
    fn reflog_get(&self, _refname: &str, _remote: Option<&str>) -> Result<KeyBuf, GetReflogError> {
        unimplemented!("null datastore, no data")
    }
    fn reflog_walk(
        &self,
        _refname: &str,
        _remote: Option<&str>,
    ) -> Result<Vec<KeyBuf>, WalkReflogError> {
        unimplemented!("null datastore, no data")
    }

    fn raw_between(&self, _start: &[u8], _end: Option<&[u8]>) -> Result<Vec<Vec<u8>>, RawBetweenError> {
        unimplemented!("null datastore, no data")
    }
}
