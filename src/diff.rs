use crate::key::TypedKey;
use crate::DataStore;
use crate::{cache, dir, file, filter};
use colored::*;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug)]
pub enum DiffTarget {
    FileSystem(PathBuf, Vec<String>, PathBuf),
    Database(TypedKey<dir::FSItem>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct DeletedDiffResult {
    path: PathBuf,
    original_key: Option<TypedKey<dir::FSItem>>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ModifiedDiffResult {
    path: PathBuf,
    original_key: TypedKey<dir::FSItem>,
    new_key: TypedKey<dir::FSItem>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct AddedDiffResult {
    path: PathBuf,
    new_key: Option<TypedKey<dir::FSItem>>,
}

#[derive(Debug)]
pub struct DiffResult {
    deleted: Vec<DeletedDiffResult>,
    added: Vec<AddedDiffResult>,
    modified: Vec<ModifiedDiffResult>,
}

#[derive(Debug, Error)]
pub enum CompareError {
    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),
    #[error("error when hashing fs item: {_0}")]
    HashError(#[from] dir::HashFsItemError),
    #[error("error when walking database items: {_0}")]
    WalkError(#[from] dir::WalkFsItemsError),
    #[error("error when walking filesystem items: {_0}")]
    RealWalkError(#[from] dir::WalkRealFsItemsError),
}

pub fn compare<'a, DS: DataStore>(
    ds: &'a mut DS,
    from: DiffTarget,
    to: Option<TypedKey<dir::FSItem>>,
    cache: impl Into<Option<&'a mut cache::SqliteCache>>,
) -> Result<DiffResult, CompareError> {
    let cache = cache.into();
    let cache = cache.as_ref();

    let from_path;
    let from_map = match from {
        DiffTarget::FileSystem(path, filters, folder_path) => {
            let exclude = filter::make_filter_fn(&filters, folder_path);
            let fs_items = dir::walk_real_fs_items(&path, &exclude)?;
            from_path = Some(path);
            either::Left(fs_items)
        }
        DiffTarget::Database(key) => {
            let db_items = dir::walk_fs_items(ds, key)?;
            from_path = None;
            either::Right(db_items)
        }
    };

    let to_map = match to {
        Some(t) => dir::walk_fs_items(ds, t)?,
        None => HashMap::new(),
    };

    let from_keys: HashSet<PathBuf> = from_map.clone().either(
        |x| x.keys().cloned().collect(),
        |x| x.keys().cloned().collect(),
    );

    let to_keys: HashSet<PathBuf> = to_map.keys().cloned().collect();

    let mut in_from_only: Vec<AddedDiffResult> = from_keys
        .difference(&to_keys)
        .map(|x| AddedDiffResult {
            path: x.clone(),
            new_key: from_map.as_ref().either(
                |y| {
                    if !y[x] {
                        // this is a file
                        match dir::hash_fs_item(
                            ds,
                            x,
                            *cache.expect("you must pass a cache if you're hashing the fs"),
                        ) {
                            Ok(h) => Some(h.into()),
                            Err(e) => panic!(e),
                        }
                    } else {
                        // directories don't have a hash
                        None
                    }
                },
                |y| Some(y[x].0),
            ),
        })
        .collect();

    let mut in_to_only: Vec<DeletedDiffResult> = to_keys
        .difference(&from_keys)
        .map(|x| DeletedDiffResult {
            path: x.clone(),
            original_key: Some(to_map[x].0),
        })
        .collect();

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
                    *cache.expect("you must pass a cache if you're hashing the fs"),
                )?
                .into();
            }
            either::Right(db_items) => f = db_items[path].0,
        }

        let t = to_map[path];

        if f != t.0 {
            let dr = ModifiedDiffResult {
                original_key: t.0,
                new_key: f,
                path: path.clone(),
            };

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

pub fn simplify(r: DiffResult) -> DiffResult {
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
    let r = simplify(r);

    use colored::*;

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

pub fn print_stat_diff_result(ds: &impl DataStore, r: DiffResult) {
    let stat = line_stat(ds, r);

    print_line_stat(stat);
}

pub fn print_patch_diff_result(ds: &impl DataStore, r: DiffResult) {
    let mut items = Vec::new();

    for added in r.added {
        if let Some(k) = added.new_key {
            println!("{}", added.path.to_string_lossy().bright_black());

            let mut data = Vec::new();
            file::read_data(ds, k.into(), &mut data).unwrap();

            let cursor = std::io::Cursor::new(data);

            for line_r in cursor.lines() {
                let line = line_r.unwrap();
                println!("{}", format!("+{}", line).green())
            }
        }
    }

    for removed in r.deleted {
        if let Some(k) = removed.original_key {
            println!("{}", removed.path.to_string_lossy().bright_black());

            let mut data = Vec::new();
            file::read_data(ds, k.into(), &mut data).unwrap();

            let cursor = std::io::Cursor::new(data);

            for line_r in cursor.lines() {
                let line = line_r.unwrap();
                println!("{}", format!("-{}", line).red())
            }
        }
    }

    for modified in r.modified {
        let mut before = Vec::new();

        ldbg!(&modified);

        file::read_data(ds, modified.original_key.into(), &mut before).unwrap();

        let mut after = Vec::new();
        file::read_data(ds, modified.new_key.into(), &mut after).unwrap();

        let before_str = String::from_utf8_lossy(&before);
        let after_str = String::from_utf8_lossy(&after);

        let lines = difference::Changeset::new(&before_str, &after_str, "\n");

        let mut removed: usize = 0;
        let mut added: usize = 0;

        for item in lines.diffs {
            match item {
                difference::Difference::Add(s) => {
                    for line in s.split('\n') {
                        added += 1;
                        println!("{}", format!("+{}", line).green())
                    }
                }
                difference::Difference::Rem(s) => {
                    for line in s.split('\n') {
                        removed += 1;
                        println!("{}", format!("-{}", line).red())
                    }
                }
                difference::Difference::Same(s) => {
                    match s.len() {
                        0..=7 => {
                            for line in s.split('\n') {
                                println!(" {}", line);
                            }
                        }
                        _ => {
                            for line in s.split('\n').take(3) {
                                println!(" {}", line);
                            }

                            println!("{}", format!(" ({} lines skipped)", s.len() - 6).bright_black());

                            for line in s.split('\n').rev().take(3).collect::<Vec<_>>().into_iter().rev() {
                                println!(" {}", line);
                            }
                        }
                    }
                }
            }
        }

        items.push(FileStatResult {
            fname: modified.path,
            added,
            removed,
        });
    }
}

#[derive(Debug)]
pub struct LineStatResult {
    items: Vec<FileStatResult>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct FileStatResult {
    fname: PathBuf,
    added: usize,
    removed: usize,
}

pub fn line_stat(ds: &impl DataStore, r: DiffResult) -> LineStatResult {
    let mut items = Vec::new();

    for added in r.added {
        if let Some(k) = added.new_key {
            items.push(FileStatResult {
                fname: added.path,
                added: line_ct(ds, k),
                removed: 0,
            });
        }
    }

    for removed in r.deleted {
        if let Some(k) = removed.original_key {
            items.push(FileStatResult {
                fname: removed.path,
                added: 0,
                removed: line_ct(ds, k),
            });
        }
    }

    for modified in r.modified {
        let mut before = Vec::new();
        file::read_data(ds, modified.original_key.into(), &mut before).unwrap();

        let mut after = Vec::new();
        file::read_data(ds, modified.new_key.into(), &mut after).unwrap();

        let before_str = String::from_utf8_lossy(&before);
        let after_str = String::from_utf8_lossy(&after);

        let lines = diff::lines(&before_str, &after_str);

        let mut removed: usize = 0;
        let mut added: usize = 0;
        for item in lines {
            match item {
                diff::Result::Left(_) => removed += 1,
                diff::Result::Right(_) => added += 1,
                diff::Result::Both(_, _) => {}
            }
        }

        items.push(FileStatResult {
            fname: modified.path,
            added,
            removed,
        });
    }

    LineStatResult { items }
}

pub fn print_line_stat(mut lsr: LineStatResult) {
    lsr.items.sort_unstable();

    for item in lsr.items {
        println!(
            "{}  +{} -{}",
            item.fname.display(),
            item.added,
            item.removed
        );
    }
}

pub fn line_ct(ds: &impl DataStore, key: TypedKey<dir::FSItem>) -> usize {
    let mut data = Vec::new();
    file::read_data(ds, key.into(), &mut data).unwrap();

    #[allow(clippy::naive_bytecount)]
    // This whole function will be cached in the store at some point, this is just for testing
    data.iter().filter(|x| **x == b'\n').count()
}

pub fn diff_result_empty(r: &DiffResult) -> bool {
    r.added.is_empty() && r.deleted.is_empty() && r.modified.is_empty()
}
