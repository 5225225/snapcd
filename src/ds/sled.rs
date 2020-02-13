use crate::DataStore;
use crate::{CanonicalizeError, GetReflogError, KeyBuf, Keyish, Reflog, WalkReflogError};
use failure::Fallible;
use std::borrow::Cow;

struct SledDS {
    db: sled::Db,

    data_tree: sled::Tree,
    state_tree: sled::Tree,
}

impl DataStore for SledDS {
    fn raw_get<'a>(&'a self, key: &[u8]) -> Fallible<Cow<'a, [u8]>> {
        Ok(Cow::Owned(self.data_tree.get(key).transpose().unwrap()?.to_vec()))
    }
    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
        self.data_tree.insert(key, data)?;

        Ok(())
    }
    fn raw_exists(&self, key: &[u8]) -> Fallible<bool> {
        Ok(self.data_tree.contains_key(key)?)
    }
    fn raw_get_state<'a>(&'a self, key: &[u8]) -> Fallible<Option<Vec<u8>>> {
        Ok(self.state_tree.get(key)?.map(|x| x.to_vec()))
    }
    fn raw_put_state<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
        self.state_tree.insert(key, data)?;

        Ok(())
    }
    fn reflog_push(&self, data: &Reflog) -> Fallible<()> {
        let tree_name = format!("REFLOG\x00{}\x01{}", data.refname, data.remote.as_deref().unwrap_or(""));

        let tree = self.db.open_tree(tree_name)?;

        let ctr = self.db.generate_id()?;

        let bytes = ctr.to_be_bytes();

        let db_key = data.key.as_db_key();

        tree.insert(&bytes, db_key)?;

        Ok(())
    }
    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> Result<KeyBuf, GetReflogError> {
        let tree_name = format!("REFLOG\x00{}\x01{}", refname, remote.as_deref().unwrap_or(""));

        let tree = self.db.open_tree(tree_name).expect("failed to open tree");

        let item = tree.iter().values().next_back().expect("no reflog item").expect("some failure");

        Ok(KeyBuf::from_db_key(&item))
    }

    fn reflog_walk(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<Vec<KeyBuf>, WalkReflogError> {
        let mut ret = Vec::new();
        let tree_name = format!("REFLOG\x00{}\x01{}", refname, remote.as_deref().unwrap_or(""));

        let tree = self.db.open_tree(tree_name).expect("failed to open tree");

        for key_r in tree.iter().values() {
            let key = key_r.unwrap();
            ret.push(KeyBuf::from_db_key(&key));
        }

        Ok(ret)
    }

    fn raw_between(&self, start: &[u8], end: Option<&[u8]>) -> Fallible<Vec<Vec<u8>>> {
        let mut result = Vec::new();
        let range = if let Some(e) = end {
            self.data_tree.range(start..e)
        } else {
            self.data_tree.range(start..)
        };

        for k_r in range.keys() {
            let k = k_r?;
            result.push(k.to_vec());
        }
        Ok(result)
    }
}
