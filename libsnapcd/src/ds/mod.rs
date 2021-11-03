pub mod null;
//pub mod sled;
pub mod sqlite;
//pub mod rocks;

use blake3::hash;

use std::borrow::Cow;

use crate::key;
use crate::keyish::Keyish;
use crate::object::Object;

#[derive(Debug)]
pub struct Reflog {
    pub refname: String,
    pub key: key::Key,
    pub remote: Option<String>,
}

static_assertions::assert_obj_safe!(DataStore);
pub trait DataStore {
    fn raw_get<'a>(&'a self, key: &[u8]) -> anyhow::Result<Cow<'a, [u8]>>;
    fn raw_put<'a>(&'a self, key: &[u8], data: &[u8]) -> anyhow::Result<()>;

    fn get_encryption_key(&self) -> &crate::crypto::EncryptionKey;
    fn get_gearhash_table(&self) -> &crate::crypto::GearHashTable;

    fn raw_exists(&self, key: &[u8]) -> anyhow::Result<bool>;

    fn raw_get_state(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>>;
    fn raw_put_state(&self, key: &[u8], data: &[u8]) -> anyhow::Result<()>;

    fn get(&self, key: key::Key) -> anyhow::Result<Cow<'_, [u8]>> {
        let crypto_key = self.get_encryption_key();

        let results = self.raw_get(&key.as_db_key())?;
        let plaintext = crypto_key.decrypt(&results);

        Ok(Cow::Owned(plaintext))
    }

    fn hash(&self, data: &[u8]) -> key::Key {
        let b3 = hash(data);
        key::Key::Blake3B(*b3.as_bytes())
    }

    fn put(&self, data: Vec<u8>) -> anyhow::Result<key::Key> {
        let crypto_key = self.get_encryption_key();
        let encrypted_data = crypto_key.encrypt(&data);
        let keybuf = self.hash(&encrypted_data);

        self.raw_put(&keybuf.as_db_key(), &encrypted_data)?;

        Ok(keybuf)
    }

    fn get_head(&self) -> anyhow::Result<Option<String>> {
        let bytes = self.raw_get_state(b"HEAD")?;

        Ok(match bytes {
            Some(b) => Some(String::from_utf8(b)?),
            None => None,
        })
    }

    fn put_head(&self, head: &str) -> anyhow::Result<()> {
        self.raw_put_state(b"HEAD", head.as_bytes())?;
        Ok(())
    }

    fn reflog_push(&self, data: &Reflog) -> anyhow::Result<()>;
    fn reflog_get(&self, refname: &str, remote: Option<&str>) -> anyhow::Result<key::Key>;
    fn reflog_walk(&self, refname: &str, remote: Option<&str>) -> anyhow::Result<Vec<key::Key>>;

    fn raw_between(&self, start: &[u8], end: Option<&[u8]>) -> anyhow::Result<Vec<Vec<u8>>>;

    fn canonicalize(&self, search: Keyish) -> anyhow::Result<key::Key> {
        let mut results: Vec<Vec<u8>>;

        let err_str;

        match search {
            Keyish::Key(_s, key) => {
                return Ok(key::Key::from_db_key(&key).unwrap());
            }
            Keyish::Range(s, start, end) => {
                err_str = s;

                results = self.raw_between(&start, end.as_deref()).unwrap();
            }
            Keyish::Reflog {
                orig,
                remote,
                keyname,
            } => return self.reflog_get(&keyname, remote.as_deref()),
        };

        match results.len() {
            0 => anyhow::bail!("not found"),
            // This is okay since we know it will have one item.
            #[allow(clippy::unwrap_used)]
            1 => Ok(key::Key::from_db_key(&results.pop().unwrap())?),
            _ => {
                let strs: Result<Vec<crate::key::Key>, crate::key::FromDbKeyError> = results
                    .into_iter()
                    .map(|x| key::Key::from_db_key(&x))
                    .collect();

                anyhow::bail!("ambiguous, found {:?}", strs)
            }
        }
    }

    fn get_obj(&self, key: key::Key) -> anyhow::Result<Object> {
        let data = self.get(key)?;

        Ok(minicbor::decode(&data)?)
    }

    fn put_obj(&self, data: &Object) -> anyhow::Result<key::Key> {
        let data = minicbor::to_vec(data)?;

        Ok(self.put(data)?)
    }
}

pub fn find_db_folder(name: &std::path::Path) -> anyhow::Result<Option<std::path::PathBuf>> {
    let cwd = std::env::current_dir()?;

    let mut d = &*cwd;

    loop {
        let mut check = d.to_path_buf();

        check.push(&name);

        if check.exists() {
            return Ok(Some(check));
        }

        d = match d.parent() {
            Some(p) => p,
            None => return Ok(None),
        };
    }
}
