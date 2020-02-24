use crate::object::ObjType;
use crate::{cache, ds};
use crate::{cache::Cache, cache::CacheKey, file, DataStore, Key, Object};
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FSItem {
    size: u64,
    itemtype: FSItemType,
    children_names: Vec<PathBuf>,
    #[serde(skip)]
    children: Vec<Key>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum FSItemType {
    Dir,
    File,
}

impl TryInto<Object<'static>> for FSItem {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Object<'static>, serde_cbor::error::Error> {
        let value = serde_cbor::value::to_value(&self)?;

        let obj = serde_cbor::to_vec(&value)?;

        let objtype = match self.itemtype {
            FSItemType::Dir => ObjType::FSItemDir,
            FSItemType::File => ObjType::FSItemFile,
        };

        Ok(Object::new_owned(obj, self.children, objtype))
    }
}

impl<'a> TryInto<FSItem> for Object<'a> {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<FSItem, serde_cbor::error::Error> {
        let item: serde_cbor::Value = serde_cbor::from_slice(&self.data())?;

        let mut fsitem: FSItem = serde_cbor::value::from_value(item)?;

        fsitem.children = self.keys().to_vec();

        Ok(fsitem)
    }
}

#[derive(Debug, Error)]
pub enum PutFsItemError {
    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),

    #[error("error putting object")]
    PutObjError(#[from] ds::PutObjError),

    #[error("error putting data")]
    PutDataError(#[from] file::PutDataError),

    #[error("serialisation error")]
    SerialisationError(#[from] serde_cbor::error::Error),
}

pub fn put_fs_item<DS: DataStore>(
    ds: &mut DS,
    path: &Path,
    filter: &dyn Fn(&DirEntry) -> bool,
) -> Result<Key, PutFsItemError> {
    let meta = std::fs::metadata(path)?;

    if meta.is_dir() {
        let mut result = Vec::new();
        let mut result_names = Vec::new();

        let entries = std::fs::read_dir(path)?;

        for entry in entries {
            match entry {
                Ok(direntry) => {
                    if filter(&direntry) {
                        result.push(put_fs_item(ds, &direntry.path(), filter)?);
                        result_names.push(direntry.file_name().into());
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        let size = result.len() as u64;

        let obj = FSItem {
            children: result,
            children_names: result_names,
            itemtype: FSItemType::Dir,
            size,
        };

        let object = obj.try_into()?;

        return Ok(ds.put_obj(&object)?);
    }

    if meta.is_file() {
        let f = std::fs::File::open(path)?;

        let reader = std::io::BufReader::new(f);

        let hash = file::put_data(ds, reader)?;

        let obj = FSItem {
            children: vec![hash],
            children_names: vec![],
            itemtype: FSItemType::File,
            size: meta.len(),
        };

        let object = obj.try_into()?;

        return Ok(ds.put_obj(&object)?);
    }

    unimplemented!("meta is not a file or a directory?")
}

#[derive(Debug, Error)]
pub enum HashFsItemError {
    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),

    #[error("put obj error: {_0}")]
    PutObjError(#[from] ds::PutObjError),

    #[error("put data error: {_0}")]
    PutDataError(#[from] file::PutDataError),

    #[error("cache error: {_0}")]
    CacheError(#[from] cache::GetCacheError),

    #[error("error when encoding object: {_0}")]
    EncodeError(#[from] serde_cbor::error::Error),

    #[error("hashing non-files is not supported")]
    NonFileError,
}

pub fn hash_fs_item<DS: DataStore, C: Cache>(
    ds: &mut DS,
    path: &Path,
    cache: &C,
) -> Result<Key, HashFsItemError> {
    let meta = std::fs::metadata(path)?;

    if meta.is_file() {
        use std::os::unix::fs::MetadataExt;

        let f = std::fs::File::open(path)?;

        let ext_metadata = f.metadata()?;
        let cache_key = CacheKey {
            mtime: ext_metadata.mtime(),
            inode: ext_metadata.ino(),
            size: ext_metadata.size(),
        };

        if let Some(h) = cache.get(cache_key)? {
            return Ok(h);
        }

        let reader = std::io::BufReader::new(f);

        let hash = file::put_data(ds, reader)?;

        let obj = FSItem {
            children: vec![hash],
            children_names: vec![],
            itemtype: FSItemType::File,
            size: meta.len(),
        };

        let object = obj.try_into()?;

        let obj_hash = ds.put_obj(&object)?;

        match cache.put(cache_key, obj_hash) {
            Ok(()) => {}
            Err(e) => log::warn!(
                "Error {:?} putting cache entry {:?} as {}",
                e,
                cache_key,
                &obj_hash
            ),
        }

        return Ok(obj_hash);
    }

    Err(HashFsItemError::NonFileError)
}

#[derive(Debug, Error)]
pub enum GetFsItemError {
    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),

    #[error("get obj error: {_0}")]
    GetObjError(#[from] ds::GetObjError),

    #[error("read data error: {_0}")]
    ReadDataError(#[from] file::ReadDataError),

    #[error("error when decoding object: {_0}")]
    DecodeError(#[from] serde_cbor::error::Error),
}

pub fn get_fs_item<DS: DataStore>(ds: &DS, key: Key, path: &Path) -> Result<(), GetFsItemError> {
    let obj = ds.get_obj(key)?;

    let fsobj: FSItem = obj.try_into()?;

    match fsobj.itemtype {
        FSItemType::Dir => {
            for (&child, name) in fsobj.children.iter().zip(fsobj.children_names.iter()) {
                get_fs_item(ds, child, &path.join(&name))?;
            }
        }
        FSItemType::File => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;

            file::read_data(ds, fsobj.children[0], &mut f)?;
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum CheckoutFsItemError {
    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),

    #[error("get obj error: {_0}")]
    GetObjError(#[from] ds::GetObjError),

    #[error("read data error: {_0}")]
    ReadDataError(#[from] file::ReadDataError),

    #[error("error when decoding object: {_0}")]
    DecodeError(#[from] serde_cbor::error::Error),

    #[error("found unimplemented file type")]
    UnimplementedFileTypeFound,
}

pub fn checkout_fs_item<DS: DataStore>(
    ds: &DS,
    key: Key,
    path: &Path,
    filter: &dyn Fn(&DirEntry) -> bool,
) -> Result<(), CheckoutFsItemError> {
    let obj = ds.get_obj(key)?;

    let fsobj: FSItem = obj.try_into()?;

    match fsobj.itemtype {
        FSItemType::Dir => {
            let db_items: HashSet<PathBuf> = fsobj.children_names.iter().cloned().collect();

            let mut fs_items = HashSet::new();

            for item in std::fs::read_dir(&path)? {
                let ok_item = item?;

                if filter(&ok_item) {
                    fs_items.insert(ok_item.file_name().into());
                }
            }

            let extra: Vec<_> = fs_items.difference(&db_items).collect();

            for item in extra.iter() {
                assert!(path.starts_with("/home/jess/src/snapcd/repo"));
                let p = path.join(item);
                let ft = std::fs::metadata(&p)?.file_type();

                if ft.is_dir() {
                    std::fs::remove_dir_all(p)?;
                } else if ft.is_file() {
                    std::fs::remove_file(p)?;
                } else if ft.is_symlink() {
                    return Err(CheckoutFsItemError::UnimplementedFileTypeFound);
                }
            }

            for (&child, name) in fsobj.children.iter().zip(fsobj.children_names.iter()) {
                std::fs::create_dir_all(&path)?;

                checkout_fs_item(ds, child, &path.join(&name), filter)?;
            }
        }
        FSItemType::File => {
            let mut f = std::fs::File::create(path)?;

            assert!(path.starts_with("/home/jess/src/snapcd/repo"));

            file::read_data(ds, fsobj.children[0], &mut f)?;
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum WalkFsItemsError {
    #[error("get obj error: {_0}")]
    GetObjError(#[from] ds::GetObjError),

    #[error("error when decoding object: {_0}")]
    DecodeError(#[from] serde_cbor::error::Error),
}

pub fn walk_fs_items<DS: DataStore>(
    ds: &DS,
    key: Key,
) -> Result<HashMap<PathBuf, (Key, bool)>, WalkFsItemsError> {
    internal_walk_fs_items(ds, key, &PathBuf::new())
}

pub fn internal_walk_fs_items<DS: DataStore>(
    ds: &DS,
    key: Key,
    path: &Path,
) -> Result<HashMap<PathBuf, (Key, bool)>, WalkFsItemsError> {
    let mut results = HashMap::new();

    let obj = ds.get_obj(key)?;

    let fsobj: FSItem = obj.try_into()?;

    match fsobj.itemtype {
        FSItemType::Dir => {
            // Same as internal_walk_real_fs_items, we don't want to add an empty entry for the
            // root.
            if !path.as_os_str().is_empty() {
                results.insert(path.to_path_buf(), (key, true));
            }

            for (&child, name) in fsobj.children.iter().zip(fsobj.children_names.iter()) {
                results.extend(internal_walk_fs_items(ds, child, &path.join(&name))?);
            }
        }
        FSItemType::File => {
            results.insert(path.to_path_buf(), (key, false));
        }
    }

    Ok(results)
}

#[derive(Debug, Error)]
pub enum WalkRealFsItemsError {
    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),

    #[error("found unimplemented file type")]
    UnimplementedFileTypeFound,
}

pub fn walk_real_fs_items(
    base_path: &Path,
    filter: &dyn Fn(&DirEntry) -> bool,
) -> Result<HashMap<PathBuf, bool>, WalkRealFsItemsError> {
    internal_walk_real_fs_items(base_path, &PathBuf::new(), filter)
}

pub fn internal_walk_real_fs_items(
    base_path: &Path,
    path: &Path,
    filter: &dyn Fn(&DirEntry) -> bool,
) -> Result<HashMap<PathBuf, bool>, WalkRealFsItemsError> {
    let mut results = HashMap::new();

    let curr_path = base_path.join(path);

    let meta = std::fs::metadata(&curr_path)?;

    if meta.is_dir() {
        let entries = std::fs::read_dir(&curr_path)?;

        // We don't want to add an empty entry for the root
        if !path.as_os_str().is_empty() {
            results.insert(path.to_path_buf(), true);
        }

        for entry in entries {
            match entry {
                Ok(direntry) => {
                    if filter(&direntry) {
                        let p = path.join(direntry.file_name());

                        results.extend(internal_walk_real_fs_items(base_path, &p, filter)?);
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        return Ok(results);
    }

    if meta.is_file() {
        results.insert(path.to_path_buf(), false);
        return Ok(results);
    }

    Err(WalkRealFsItemsError::UnimplementedFileTypeFound)
}
