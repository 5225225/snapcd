use crate::{cache, dir, filter};
use crate::{DataStore, KeyBuf};
use failure::Fallible;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub enum DiffTarget {
    FileSystem(PathBuf, Vec<String>, PathBuf),
    Database(KeyBuf),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct DeletedDiffResult {
    path: PathBuf,
    original_key: KeyBuf,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ModifiedDiffResult {
    path: PathBuf,
    original_key: KeyBuf,
    new_key: KeyBuf,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct AddedDiffResult {
    path: PathBuf,
    new_key: KeyBuf,
}

pub struct DiffResult {
    deleted: Vec<DeletedDiffResult>,
    added: Vec<AddedDiffResult>,
    modified: Vec<ModifiedDiffResult>,
}

pub fn compare<DS: DataStore>(
    ds: &mut DS,
    from: DiffTarget,
    to: Option<KeyBuf>,
    cache: &mut cache::SqliteCache,
) -> Fallible<DiffResult> {
    let from_path;
    let from_map = match from {
        DiffTarget::FileSystem(path, filters, folder_path) => {
            let exclude = filter::make_filter_fn(&filters, folder_path);
            let fs_items = dir::walk_real_fs_items(&path, &exclude)?;
            from_path = Some(path.clone());
            either::Left(fs_items)
        }
        DiffTarget::Database(key) => {
            let db_items = dir::walk_fs_items(ds, &key)?;
            from_path = None;
            either::Right(db_items)
        }
    };

    let to_map = match &to {
        Some(t) => dir::walk_fs_items(ds, &t)?,
        None => HashMap::new(),
    };

    let from_keys: HashSet<PathBuf> = from_map.clone().either(
        |x| x.keys().cloned().collect(),
        |x| x.keys().cloned().collect(),
    );

    let to_keys: HashSet<PathBuf> = to_map.keys().cloned().collect();

    let mut in_from_only: Vec<DeletedDiffResult> = to_keys.difference(&from_keys).map(|x| DeletedDiffResult{
        path: x.clone(),
        original_key: from_map.either(
            |y| dir::hash_fs_item(ds, x, cache).expect("failed to hash"),
            |y| y[x].0),
    }).collect();

    let mut in_to_only: Vec<AddedDiffResult> = from_keys.difference(&to_keys).map(|x| AddedDiffResult {
        path: x.clone(),
        new_key: to_map[x].0.clone(),
    }).collect();

    let in_both: Vec<_> = from_keys.intersection(&to_keys).collect();

    let mut modified = Vec::new();

    for path in in_both {
        let f;
        match &from_map {
            either::Left(fs_items) => {
                if fs_items[path] {
                    continue;
                }

                f = dir::hash_fs_item(
                    ds,
                    &from_path
                        .as_ref()
                        .expect("should have been populated")
                        .join(path),
                    cache,
                )?;
            }
            either::Right(db_items) => f = db_items[path].0.clone(),
        }

        let t = to_map[path].clone();

        if f != t.0 {
            let dr = ModifiedDiffResult {
                original_key: f,
                new_key: t.0,
                path: path.clone(),
            };

            modified.push(dr);
        }
    }

    in_from_only.sort_unstable();
    in_to_only.sort_unstable();
    modified.sort_unstable();

    Ok(DiffResult {
        deleted: in_from_only,
        added: in_to_only,
        modified: modified,
    })
}

pub fn simplify(r: DiffResult) -> DiffResult {
    let mut deleted = Vec::new();
    let mut added = Vec::new();

    if !r.added.is_empty() {
        for p in r.added {
            if added.iter().any(|x| p.path.starts_with(x.path)) {
                added.push(p);
            }
        }
    }

    if !r.deleted.is_empty() {
        for p in r.deleted {
            if !deleted.iter().any(|x| p.starts_with(x)) {
                deleted.push(p);
            }
        }
    }

    DiffResult {added, modified: r.modified, deleted}

    // Directories can't be modified, so we don't need to simplify here
}

pub fn print_diff_result(r: DiffResult) {
    let r = simplify(r);

    use colored::*;

    if !r.added.is_empty() {
        println!("{}", "added:".green());
        for p in r.added {
            println!("  {}", p.path.display());
        }
    }
    if !r.deleted.is_empty() {
        println!("{}", "deleted:".red());
        for p in r.deleted {
            println!("  {}", p.path.display());
        }
    }

    // Directories can't be modified, so we don't need to simplify here
    if !r.modified.is_empty() {
        println!("{}", "modified:".blue());
        for p in r.modified {
            println!("  {}", p.path.display());
        }
    }
}

pub fn diff_result_empty(r: &DiffResult) -> bool {
    r.added.is_empty() && r.deleted.is_empty() && r.modified.is_empty()
}
