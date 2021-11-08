use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    cache::{self, Cache},
    ds::DataStore,
    entry::Entry,
    file,
    key::Key,
    object::Object,
};

/// `full_path` is the path relative to the root.
/// If the put started outside of the root, it will be None
pub fn put_fs_item<DS: DataStore>(
    ds: &mut DS,
    entry: &Entry,
    path: &Path,
    filter: &dyn Fn(&Path) -> bool,
) -> anyhow::Result<Key> {
    match entry {
        Entry::Dir(d) => {
            let mut result = Vec::new();

            let entries = d.entries()?;

            for entry in entries {
                match entry {
                    Ok(direntry) => {
                        let mut p = path.to_path_buf();
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
                                put_fs_item(ds, &Entry::from_direntry(&direntry), &p, filter)?,
                                is_dir,
                            ));
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            let obj = Object::FsItemDir { children: result };

            Ok(ds.put_obj(&obj)?)
        }
        Entry::File(f) => {
            let reader = std::io::BufReader::new(f);

            let hash = file::put_data(ds, reader)?;

            // TODO: we should be able to ask put_data how big the file was

            let obj = Object::FsItemFile {
                blob_tree: hash,
                size: f.metadata().unwrap().len(),
            };

            Ok(ds.put_obj(&obj)?)
        }
    }
}

pub fn hash_fs_item<DS: DataStore, C: Cache>(
    ds: &mut DS,
    path: &Path,
    cache: &C,
) -> anyhow::Result<Key> {
    let meta = std::fs::metadata(path)?;

    if meta.is_file() {
        use std::os::unix::fs::MetadataExt;

        let f = std::fs::File::open(path)?;

        let ext_metadata = f.metadata()?;
        let cache_key = cache::Key {
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

    anyhow::bail!("Not a file");
}

pub fn get_fs_item<DS: DataStore>(
    ds: &DS,
    key: Key,
    path: &Path,
    auth: cap_std::AmbientAuthority,
) -> anyhow::Result<()> {
    let ty = ds.get_obj(key)?;
    match ty {
        Object::FsItemDir { .. } => {
            std::fs::create_dir_all(path)?;
            let d = cap_std::fs::Dir::open_ambient_dir(path, auth)?;

            get_fs_item_dir(ds, key, &d)?;
        }
        Object::FsItemFile { .. } => {
            let stdf = std::fs::File::create(path)?;
            let f = cap_std::fs::File::from_std(stdf, auth);
            get_fs_item_file(ds, key, &f)?;
        }
        Object::Commit { tree, .. } => {
            return get_fs_item(ds, tree, path, auth);
        }
        o => panic!("Tried to extract unrecognised object {:?}", o),
    }

    Ok(())
}

#[allow(clippy::module_name_repetitions)]
pub fn get_fs_item_dir<DS: DataStore>(
    ds: &DS,
    key: Key,
    path: &cap_std::fs::Dir,
) -> anyhow::Result<()> {
    let obj = ds.get_obj(key)?;

    match obj {
        Object::FsItemDir { children } => {
            for (name, key, is_dir) in &children {
                dbg!(&name, &key, &is_dir);
                if *is_dir {
                    path.create_dir(name)?;
                    get_fs_item_dir(ds, *key, &path.open_dir(name).unwrap())?;
                } else {
                    get_fs_item_file(ds, *key, &path.create(name).unwrap())?;
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
) -> anyhow::Result<()> {
    let obj = ds.get_obj(key)?;

    match obj {
        Object::FsItemFile { blob_tree, size: _ } => {
            file::read_data(ds, blob_tree, &mut path)?;
        }
        _ => panic!("cannot handle this type"),
    }

    Ok(())
}

pub fn checkout_fs_item<DS: DataStore>(
    ds: &DS,
    key: Key,
    path: &Path,
    filter: &dyn Fn(&Path) -> bool,
) -> anyhow::Result<()> {
    let obj = ds.get_obj(key)?;

    match obj {
        Object::FsItemDir { children } => {
            let db_items: HashSet<PathBuf> = children.iter().map(|x| x.0.clone()).collect();

            let mut fs_items = HashSet::new();

            for item in std::fs::read_dir(&path)? {
                let ok_item = item?;

                if filter(path) {
                    fs_items.insert(ok_item.file_name().into());
                }
            }

            let extra: Vec<_> = fs_items.difference(&db_items).collect();

            for item in &extra {
                assert!(path.starts_with("/home/jess/src/snapcd/repo"));
                let p = path.join(item);
                let ft = std::fs::metadata(&p)?.file_type();

                if ft.is_dir() {
                    std::fs::remove_dir_all(p)?;
                } else if ft.is_file() {
                    std::fs::remove_file(p)?;
                } else if ft.is_symlink() {
                    anyhow::bail!("symlinks not supported");
                }
            }

            for (name, key, _is_dir) in &children {
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

pub fn walk_fs_items<DS: DataStore>(
    ds: &DS,
    key: Key,
) -> anyhow::Result<HashMap<PathBuf, (Key, bool)>> {
    internal_walk_fs_items(ds, key, &PathBuf::new())
}

pub fn internal_walk_fs_items<DS: DataStore>(
    ds: &DS,
    key: Key,
    path: &Path,
) -> anyhow::Result<HashMap<PathBuf, (Key, bool)>> {
    let mut results = HashMap::new();

    let obj = ds.get_obj(key)?;

    match obj {
        Object::FsItemDir { children } => {
            // Same as internal_walk_real_fs_items, we don't want to add an empty entry for the
            // root.
            if !path.as_os_str().is_empty() {
                results.insert(path.to_path_buf(), (key, true));
            }

            for (name, key, _is_dir) in children {
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

pub fn walk_real_fs_items(
    base_path: &Path,
    filter: &dyn Fn(&Path) -> bool,
) -> anyhow::Result<HashMap<PathBuf, bool>> {
    internal_walk_real_fs_items(base_path, &PathBuf::new(), filter)
}

pub fn internal_walk_real_fs_items(
    base_path: &Path,
    path: &Path,
    filter: &dyn Fn(&Path) -> bool,
) -> anyhow::Result<HashMap<PathBuf, bool>> {
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

    anyhow::bail!("symlinks not supported");
}
