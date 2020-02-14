use crate::DataStore;
use crate::{GetReflogError, KeyBuf, Reflog, WalkReflogError};
use failure::Fallible;
use std::borrow::Cow;
use std::path::Path;

pub struct SledDS {
    db: sled::Db,

    data_tree: sled::Tree,
    state_tree: sled::Tree,

    batch: std::cell::RefCell<sled::Batch>,
}

impl std::ops::Drop for SledDS {
    fn drop(&mut self) {
        self.commit();
    }
}

impl SledDS {
    pub fn new_tmp() -> Fallible<Self> {
        let db = sled::Config::default().temporary(true).open()?;

        let data_tree = db.open_tree("DATA")?;
        let state_tree = db.open_tree("STATE")?;

        Ok(Self {
            db,
            data_tree,
            state_tree,
            batch: Default::default(),
        })
    }

    pub fn new(path: &Path) -> Fallible<Self> {
        let db = sled::open(path)?;

        let data_tree = db.open_tree("DATA")?;
        let state_tree = db.open_tree("STATE")?;

        Ok(Self {
            db,
            data_tree,
            state_tree,
            batch: Default::default(),
        })
    }

    fn commit(&self) {
        self.data_tree
            .apply_batch(self.batch.replace(Default::default()))
            .unwrap();
    }
}

impl DataStore for SledDS {
    fn raw_get<'a>(&'a self, key: &[u8]) -> Fallible<Cow<'a, [u8]>> {
        self.commit();

        Ok(Cow::Owned(
            self.data_tree.get(key).transpose().unwrap()?.to_vec(),
        ))
    }
    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
        self.batch.borrow_mut().insert(key, data);

        Ok(())
    }
    fn raw_exists(&self, key: &[u8]) -> Fallible<bool> {
        self.commit();

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
        let tree_name = format!(
            "REFLOG\x00{}\x01{}",
            data.refname,
            data.remote.as_deref().unwrap_or("")
        );

        let tree = self.db.open_tree(tree_name)?;

        let ctr = self.db.generate_id()?;

        let bytes = ctr.to_be_bytes();

        let db_key = data.key.as_db_key();

        tree.insert(&bytes, db_key)?;

        Ok(())
    }
    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> Result<KeyBuf, GetReflogError> {
        let tree_name = format!(
            "REFLOG\x00{}\x01{}",
            refname,
            remote.as_deref().unwrap_or("")
        );

        let tree = self.db.open_tree(tree_name).expect("failed to open tree");

        let item = tree
            .iter()
            .values()
            .next_back()
            .expect("no reflog item")
            .expect("some failure");

        Ok(KeyBuf::from_db_key(&item))
    }

    fn reflog_walk(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<Vec<KeyBuf>, WalkReflogError> {
        let mut ret = Vec::new();
        let tree_name = format!(
            "REFLOG\x00{}\x01{}",
            refname,
            remote.as_deref().unwrap_or("")
        );

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
