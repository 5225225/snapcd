use crate::{cache, dir, filter};
use crate::{DataStore, KeyBuf};
use failure::Fallible;
use std::collections::HashSet;
use std::path::PathBuf;

pub enum DiffTarget {
    FileSystem(PathBuf, Vec<String>, PathBuf),
    Database(KeyBuf),
}

pub struct DiffResult {
    deleted: Vec<PathBuf>,
    added: Vec<PathBuf>,
    modified: Vec<PathBuf>,
}

pub fn compare<DS: DataStore>(
    ds: &mut DS,
    from: DiffTarget,
    to: KeyBuf,
    cache: &mut cache::SqliteCache,
) -> Fallible<DiffResult> {
    let from_path;
    let from_map = match from {
        DiffTarget::FileSystem(path, filters, folder_path) => {
            let exclude = filter::make_filter_fn(&filters, &Some(folder_path));
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

    let to_map = dir::walk_fs_items(ds, &to)?;

    let from_keys: HashSet<PathBuf> = from_map.clone().either(
        |x| x.keys().cloned().collect(),
        |x| x.keys().cloned().collect(),
    );
    let to_keys: HashSet<PathBuf> = to_map.keys().cloned().collect();

    let mut in_from_only: Vec<_> = to_keys.difference(&from_keys).cloned().collect();
    let mut in_to_only: Vec<_> = from_keys.difference(&to_keys).cloned().collect();
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
            modified.push(path.clone());
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

pub fn print_diff_result(r: DiffResult) {
    use colored::*;

    if !r.added.is_empty() {
        let mut already_added: HashSet<PathBuf> = HashSet::new();
        println!("{}", "added:".green());
        for p in r.added {
            if !already_added.iter().any(|x| p.starts_with(x)) {
                println!("  {}", p.display());
                already_added.insert(p);
            }
        }
    }
    if !r.deleted.is_empty() {
        let mut already_added: HashSet<PathBuf> = HashSet::new();
        println!("{}", "deleted:".red());
        for p in r.deleted {
            if !already_added.iter().any(|x| p.starts_with(x)) {
                println!("  {}", p.display());
                already_added.insert(p);
            }
        }
    }

    // Directories can't be modified, so we don't need to simplify here
    if !r.modified.is_empty() {
        println!("{}", "modified:".blue());
        for p in r.modified {
            println!("  {}", p.display());
        }
    }
}
