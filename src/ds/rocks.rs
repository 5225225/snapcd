use std::path::Path;
use crate::DataStore;
use crate::{GetReflogError, KeyBuf, Reflog, WalkReflogError};
use failure::Fallible;
use std::borrow::Cow;
use std::cell::RefCell;

pub struct RocksDS {
    db: rocksdb::DB,
    batch: RefCell<rocksdb::WriteBatch>,
}

impl RocksDS {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, rocksdb::Error> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        let db = rocksdb::DB::open(&opts, path)?;



        Ok(Self {
            db,
            batch: Default::default(),
        })
    }

    fn commit(&self) -> Result<(), rocksdb::Error> {
        self.db.write(self.batch.replace(Default::default()))
    }
}

impl DataStore for RocksDS {
    fn raw_get<'a>(&'a self, key: &[u8]) -> Fallible<Cow<'a, [u8]>> {
        self.commit()?;

        let mut new_key = Vec::with_capacity(key.len() + 1);

        new_key.push(1);
        new_key.extend_from_slice(key);

        Ok(Cow::Owned(
            self.db.get(&new_key).transpose().unwrap()?.to_vec(),
        ))
    }
    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
        let mut new_key = Vec::with_capacity(key.len() + 1);

        new_key.push(1);
        new_key.extend_from_slice(key);

        self.batch.borrow_mut().put(new_key, data);

        Ok(())
    }
    fn raw_exists(&self, key: &[u8]) -> Fallible<bool> {
        self.commit()?;

        let mut new_key = Vec::with_capacity(key.len() + 1);

        new_key.push(1);
        new_key.extend_from_slice(key);

        Ok(self.db.get(&new_key)?.is_some())
    }
    fn raw_get_state<'a>(&'a self, key: &[u8]) -> Fallible<Option<Vec<u8>>> {
        let mut new_key = Vec::with_capacity(key.len() + 1);

        new_key.push(2);
        new_key.extend_from_slice(key);

        Ok(self.db.get(key)?.map(|x| x.to_vec()))
    }
    fn raw_put_state<'a>(&'a self, key: &[u8], data: &[u8]) -> Fallible<()> {
        let mut new_key = Vec::with_capacity(key.len() + 1);

        new_key.push(2);
        new_key.extend_from_slice(key);

        self.batch.borrow_mut().put(&new_key, data)?;

        Ok(())
    }
    fn reflog_push(&self, data: &Reflog) -> Fallible<()> {
        todo!()
    }
    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> Result<KeyBuf, GetReflogError> {
        todo!()
    }

    fn reflog_walk(
        &self,
        refname: &str,
        remote: Option<&str>,
    ) -> Result<Vec<KeyBuf>, WalkReflogError> {
        todo!()
    }

    fn raw_between(&self, start: &[u8], end: Option<&[u8]>) -> Fallible<Vec<Vec<u8>>> {
        use rocksdb::{Direction, IteratorMode};

        let mut result = Vec::new();

        let mut new_key = Vec::with_capacity(start.len() + 1);

        new_key.push(1);
        new_key.extend_from_slice(start);

        let iter = self.db.full_iterator(IteratorMode::From(&new_key, Direction::Forward));


        if let Some(e) = end {
            for (key, _value) in iter.filter(|x| x.0.starts_with(e)).fuse() {
                result.push(key.to_vec());
            }
        } else {
            for (key, _value) in iter {
                result.push(key.to_vec());
            }
        };

        Ok(result)
    }
}
