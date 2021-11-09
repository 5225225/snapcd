use std::{collections::HashSet, path::PathBuf};

use crate::{dir, ds::DataStore, key::Key};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub enum DiffTarget {
    FileSystem(PathBuf, Vec<String>, PathBuf),
    Database(Key),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
struct DeletedDiffResult {
    path: PathBuf,
    is_dir: bool,
    original_key: Option<Key>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ModifiedDiffResult {
    path: PathBuf,
    original_key: Key,
    new_key: Key,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct AddedDiffResult {
    path: PathBuf,
    is_dir: bool,
    new_key: Option<Key>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct DiffResult {
    deleted: Vec<DeletedDiffResult>,
    added: Vec<AddedDiffResult>,
    modified: Vec<ModifiedDiffResult>,
}

pub fn compare(ds: &mut impl DataStore, from: Key, to: Key) -> anyhow::Result<DiffResult> {
    let db_items = dir::walk_fs_items(ds, from)?;

    let to_map = dir::walk_fs_items(ds, to)?;

    let from_keys: HashSet<PathBuf> = db_items.keys().cloned().collect();

    let to_keys: HashSet<PathBuf> = to_map.keys().cloned().collect();

    let mut in_from_only: Vec<AddedDiffResult> = from_keys
        .difference(&to_keys)
        .map(|x| AddedDiffResult {
            path: x.clone(),
            new_key: Some(db_items[x].0),
            is_dir: db_items[x].1,
        })
        .collect();

    let mut in_to_only: Vec<DeletedDiffResult> = to_keys
        .difference(&from_keys)
        .map(|x| DeletedDiffResult {
            path: x.clone(),
            original_key: Some(to_map[x].0),
            is_dir: to_map[x].1,
        })
        .collect();

    let in_both: Vec<_> = from_keys.intersection(&to_keys).collect();

    let mut modified = Vec::new();

    for path in in_both {
        let f = db_items[path].0;

        let t = to_map[path];

        if f != t.0 {
            let dr = ModifiedDiffResult {
                original_key: t.0,
                new_key: f,
                path: path.clone(),
            };

            tracing::debug!(
                "diff: pushing {:?} to modified because {} != {}",
                dr,
                f,
                t.0
            );

            modified.push(dr);
        }
    }

    in_from_only.sort_unstable();
    in_to_only.sort_unstable();
    modified.sort_unstable();

    Ok(DiffResult {
        deleted: in_to_only,
        added: in_from_only,
        modified,
    })
}

#[must_use]
fn simplify(r: DiffResult) -> DiffResult {
    let mut deleted = Vec::new();
    let mut added = Vec::new();

    if !r.added.is_empty() {
        for p in r.added {
            if !added
                .iter()
                .any(|x: &AddedDiffResult| p.path.starts_with(&x.path))
            {
                added.push(p);
            }
        }
    }

    if !r.deleted.is_empty() {
        for p in r.deleted {
            if !deleted
                .iter()
                .any(|x: &DeletedDiffResult| p.path.starts_with(&x.path))
            {
                deleted.push(p);
            }
        }
    }

    DiffResult {
        added,
        modified: r.modified,
        deleted,
    }

    // Directories can't be modified, so we don't need to simplify here
}

pub fn print_diff_result(r: DiffResult) {
    use colored::Colorize;

    let r = simplify(r);

    if !r.added.is_empty() {
        println!("{}", "added:".green());
        for p in r.added {
            match p.new_key {
                Some(_) => println!("  {}", p.path.display()),
                None => println!("  {}/", p.path.display().to_string().bold()),
            }
        }
    }
    if !r.deleted.is_empty() {
        println!("{}", "deleted:".red());
        for p in r.deleted {
            match p.original_key {
                Some(_) => println!("  {}", p.path.display()),
                None => println!("  {}/", p.path.display().to_string().bold()),
            }
        }
    }

    if !r.modified.is_empty() {
        println!("{}", "modified:".blue());
        for p in r.modified {
            println!("  {}", p.path.display());
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
struct FileStatResult {
    fname: PathBuf,
    added: usize,
    removed: usize,
}
