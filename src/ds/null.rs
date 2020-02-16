use crate::{GetReflogError, KeyBuf, Reflog, WalkReflogError};
use failure::Fallible;
use std::borrow::Cow;

pub struct NullDS;

impl crate::DataStore for NullDS {
    fn raw_get<'a>(&'a self, _key: &[u8]) -> Fallible<Cow<'a, [u8]>> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put<'a>(&'a self, _key: &[u8], _data: &[u8]) -> Fallible<()> {
        Ok(())
    }
    fn raw_exists(&self, _key: &[u8]) -> Fallible<bool> {
        unimplemented!("null datastore, no data")
    }
    fn raw_get_state<'a>(&'a self, _key: &[u8]) -> Fallible<Option<Vec<u8>>> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put_state<'a>(&'a self, _key: &[u8], _data: &[u8]) -> Fallible<()> {
        Ok(())
    }
    fn reflog_push(&self, _data: &Reflog) -> Fallible<()> {
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

    fn raw_between(&self, _start: &[u8], _end: Option<&[u8]>) -> Fallible<Vec<Vec<u8>>> {
        unimplemented!("null datastore, no data")
    }
}
