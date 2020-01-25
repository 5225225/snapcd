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

    let in_from_only = to_keys.difference(&from_keys).cloned().collect();
    let in_to_only = from_keys.difference(&to_keys).cloned().collect();
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

        //        from.either(|x| x[path], |x| x[path]);
        //        let x: () = path;
        //        let db_key = &from.either(|x| dir::hash_fs_item(ds, item, cache), |x| x[item].0);

        //        if to[*item] {
        //            continue;
        //        }

        /*

        let fs_item_key = dir::hash_fs_item(ds, &path.join(item), &state.cache)?;

        if db_key.0 != fs_item_key {
            println!("modified: {}", item.display());
        }
        */
    }

    Ok(DiffResult {
        deleted: in_from_only,
        added: in_to_only,
        modified: modified,
    })
}

pub fn print_diff_result(r: DiffResult) {
    use colored::*;

    if !r.added.is_empty() {
        println!("{}", "added:".green());
        for p in r.added {
            println!("  {}", p.display());
        }
    }
    if !r.deleted.is_empty() {
        println!("{}", "deleted:".red());
        for p in r.deleted {
            println!("  {}", p.display());
        }
    }
    if !r.modified.is_empty() {
        println!("{}", "modified:".blue());
        for p in r.modified {
            println!("  {}", p.display());
        }
    }
}
