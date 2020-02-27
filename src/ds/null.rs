use crate::commit;
use crate::ds;
use crate::ds::{
    GetReflogError, RawBetweenError, RawExistsError, RawGetError, RawGetStateError, RawPutError,
    RawPutStateError, ReflogPushError, WalkReflogError,
};
use crate::key::TypedKey;
use crate::Reflog;
use std::borrow::Cow;

#[derive(Debug)]
pub struct NullDS;

impl ds::Transactional for NullDS {}

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
    fn reflog_get(
        &self,
        _refname: &str,
        _remote: Option<&str>,
    ) -> Result<TypedKey<commit::Commit>, GetReflogError> {
        unimplemented!("null datastore, no data")
    }
    fn reflog_walk(
        &self,
        _refname: &str,
        _remote: Option<&str>,
    ) -> Result<Vec<TypedKey<commit::Commit>>, WalkReflogError> {
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
