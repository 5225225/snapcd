use crate::crypto;
use crate::ds::{
    GetReflogError, RawBetweenError, RawExistsError, RawGetError, RawGetStateError, RawPutError,
    RawPutStateError, ReflogPushError, WalkReflogError,
};
use crate::key::Key;
use crate::Reflog;
use std::borrow::Cow;

#[derive(Debug)]
pub struct NullDs {
    encryption_key: crypto::EncryptionKey,
    gearhash_table: crypto::GearHashTable,
}

impl NullDs {
    pub fn new() -> Self {
        let zk = crypto::RepoKey::zero_key();
        let encryption_key = zk.derive_encryption_key();
        let gearhash_table = zk.derive_gearhash_table();

        NullDs {
            encryption_key,
            gearhash_table,
        }
    }
}

impl Default for NullDs {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::DataStore for NullDs {
    fn get_encryption_key(&self) -> &crypto::EncryptionKey {
        &self.encryption_key
    }

    fn get_gearhash_table(&self) -> &crypto::GearHashTable {
        &self.gearhash_table
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
