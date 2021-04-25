use crate::entry::Entry;
use crate::{cache, ds};
use crate::{cache::Cache, cache::CacheKey, file, key::Key, DataStore, Object};
use std::collections::{HashMap, HashSet};
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PutFsItemError {
    #[error("io error: {_0}")]
    IoError(#[from] std::io::Error),

    #[error("error putting object")]
    PutObjError(#[from] ds::PutObjError),

    #[error("error putting data")]
    PutDataError(#[from] file::PutDataError),

    #[error("serialisation error")]
    SerialisationError(#[from] serde_cbor::error::Error),
}

/// `full_path` is the path relative to the root.
/// If the put started outside of the root, it will be None
pub fn put_fs_item<DS: DataStore>(
    ds: &mut DS,
    entry: &Entry,
    path: PathBuf,
    filter: &dyn Fn(&std::path::Path) -> bool,
) -> Result<Key, PutFsItemError> {
    match entry {
        Entry::Dir(d) => {
            let mut result = Vec::new();

            let entries = d.entries()?;

            for entry in entries {
                match entry {
                    Ok(direntry) => {
                        let mut p = path.clone();
                        p.push(direntry.file_name());

                        let is_dir;
                        let dft = direntry.file_type().unwrap();
                        if dft.is_dir() {
                            is_dir = true;
                        } else if dft.is_file() {
                            is_dir = false;
                        } else {
                            panic!("unimplemented file type {:?} for {:?}", dft, direntry);
                        }

                        if filter(&p) {
                            result.push((
                                direntry.file_name().into(),
                                put_fs_item(ds, &Entry::from_direntry(&direntry), p, filter)?,
                                is_dir,
                            ));
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            let obj = Object::FsItemDir { children: result };

            return Ok(ds.put_obj(&obj)?);
        }
        Entry::File(f) => {
            let reader = std::io::BufReader::new(f);

            let hash = file::put_data(ds, reader)?;

            // TODO: we should be able to ask put_data how big the file was

            let obj = Object::FsItemFile {
                blob_tree: hash,
                size: f.metadata().unwrap().len(),
            };

            return Ok(ds.put_obj(&obj)?);
        }
    }
}

#[derive(Debug, Error)]
pub enum HashFsItemError {
    #[error("io error: {_0}")]
    IoError(#[from] std::io::Error),

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

        let obj = Object::FsItemFile {
            blob_tree: hash,
            size: meta.len(),
        };

        let obj_hash = ds.put_obj(&obj)?;

        match cache.put(cache_key, obj_hash) {
            Ok(()) => {}
            Err(e) => tracing::warn!(
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
    IoError(#[from] std::io::Error),

    #[error("get obj error: {_0}")]
    GetObjError(#[from] ds::GetObjError),

    #[error("read data error: {_0}")]
    ReadDataError(#[from] file::ReadDataError),

    #[error("error when decoding object: {_0}")]
    DecodeError(#[from] serde_cbor::error::Error),
}

pub fn get_fs_item<DS: DataStore>(
    ds: &DS,
    key: Key,
    mut path: Entry,
) -> Result<(), GetFsItemError> {
    match path {
        Entry::Dir(f) => get_fs_item_dir(ds, key, &f),
        Entry::File(f) => get_fs_item_file(ds, key, &f),
    }
}

pub fn get_fs_item_dir<DS: DataStore>(
    ds: &DS,
    key: Key,
    mut path: &cap_std::fs::Dir,
) -> Result<(), GetFsItemError> {
    let obj = ds.get_obj(key)?;

    match obj {
        Object::FsItemDir { children } => {
            for (name, key, is_dir) in children.iter() {
                if *is_dir {
                    get_fs_item_dir(ds, *key, &path.open_dir(name).unwrap())?;
                } else {
                    get_fs_item_file(ds, *key, &path.open(name).unwrap())?;
                }
            }
        }
        _ => panic!("cannot handle this type"),
    }

    Ok(())
}

pub fn get_fs_item_file<DS: DataStore>(
    ds: &DS,
    key: Key,
    mut path: &cap_std::fs::File,
) -> Result<(), GetFsItemError> {
    let obj = ds.get_obj(key)?;

    match obj {
        Object::FsItemFile { blob_tree, size: _ } => {
            file::read_data(ds, blob_tree, &mut path)?;
        }
        _ => panic!("cannot handle this type"),
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum CheckoutFsItemError {
    #[error("io error: {_0}")]
    IoError(#[from] std::io::Error),

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
    filter: &dyn Fn(&std::path::Path) -> bool,
) -> Result<(), CheckoutFsItemError> {
    let obj = ds.get_obj(key)?;

    match obj {
        Object::FsItemDir { children } => {
            let db_items: HashSet<PathBuf> = children.iter().map(|x| x.0.clone()).collect();

            let mut fs_items = HashSet::new();

            for item in std::fs::read_dir(&path)? {
                let ok_item = item?;

                if filter(&path) {
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

            for (name, key, is_dir) in &children {
                std::fs::create_dir_all(&path)?;

                checkout_fs_item(ds, *key, &path.join(&name), filter)?;
            }
        }
        Object::FsItemFile { blob_tree, size: _ } => {
            let mut f = std::fs::File::create(path)?;

            assert!(path.starts_with("/home/jess/src/snapcd/repo"));

            file::read_data(ds, blob_tree, &mut f)?;
        }
        _ => panic!("cannot handle this type"),
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

    match obj {
        Object::FsItemDir { children } => {
            // Same as internal_walk_real_fs_items, we don't want to add an empty entry for the
            // root.
            if !path.as_os_str().is_empty() {
                results.insert(path.to_path_buf(), (key, true));
            }

            for (name, key, is_dir) in children {
                results.extend(internal_walk_fs_items(ds, key, &path.join(&name))?);
            }
        }
        Object::FsItemFile { .. } => {
            results.insert(path.to_path_buf(), (key, false));
        }
        e => panic!("cannot handle object {:?}", e),
    }

    Ok(results)
}

#[derive(Debug, Error)]
pub enum WalkRealFsItemsError {
    #[error("io error: {_0}")]
    IoError(#[from] std::io::Error),

    #[error("found unimplemented file type")]
    UnimplementedFileTypeFound,
}

pub fn walk_real_fs_items(
    base_path: &Path,
    filter: &dyn Fn(&std::path::Path) -> bool,
) -> Result<HashMap<PathBuf, bool>, WalkRealFsItemsError> {
    internal_walk_real_fs_items(base_path, &PathBuf::new(), filter)
}

pub fn internal_walk_real_fs_items(
    base_path: &Path,
    path: &Path,
    filter: &dyn Fn(&std::path::Path) -> bool,
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
                    let p = path.join(direntry.file_name());

                    if filter(&p) {
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
