use std::borrow::Cow;

use crate::{
    crypto,
    ds::{GetReflogError, Reflog},
    key::Key,
};

#[derive(Debug)]
pub struct Null {
    encryption_key: crypto::EncryptionKey,
    gearhash_table: crypto::GearHashTable,
}

impl Null {
    #[must_use]
    pub fn new() -> Self {
        let zk = crypto::RepoKey::ZERO;
        let encryption_key = zk.derive_encryption_key();
        let gearhash_table = zk.derive_gearhash_table();

        Self {
            encryption_key,
            gearhash_table,
        }
    }
}

impl Default for Null {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::ds::DataStore for Null {
    fn get_encryption_key(&self) -> &crypto::EncryptionKey {
        &self.encryption_key
    }

    fn get_gearhash_table(&self) -> &crypto::GearHashTable {
        &self.gearhash_table
    }

    fn raw_get<'a>(&'a self, _key: &[u8]) -> anyhow::Result<Cow<'a, [u8]>> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put<'a>(&'a self, _key: &[u8], _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
    fn raw_exists(&self, _key: &[u8]) -> anyhow::Result<bool> {
        unimplemented!("null datastore, no data")
    }
    fn raw_get_state(&self, _key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        unimplemented!("null datastore, no data")
    }
    fn raw_put_state(&self, _key: &[u8], _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
    fn reflog_push(&self, _data: &Reflog) -> anyhow::Result<()> {
        Ok(())
    }
    fn reflog_get(&self, _refname: &str, _remote: Option<&str>) -> Result<Key, GetReflogError> {
        unimplemented!("null datastore, no data")
    }
    fn reflog_walk(&self, _refname: &str, _remote: Option<&str>) -> anyhow::Result<Vec<Key>> {
        unimplemented!("null datastore, no data")
    }

    fn raw_between(&self, _start: &[u8], _end: Option<&[u8]>) -> anyhow::Result<Vec<Vec<u8>>> {
        unimplemented!("null datastore, no data")
    }
}
