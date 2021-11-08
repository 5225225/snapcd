use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::{cache, dir, ds::DataStore, file, filter, key::Key};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub enum DiffTarget {
    FileSystem(PathBuf, Vec<String>, PathBuf),
    Database(Key),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct DeletedDiffResult {
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

// i don't care, i'll probably refactor this anyways :)
#[allow(clippy::too_many_lines)]
pub fn compare<'a>(
    ds: &'a mut impl DataStore,
    from: DiffTarget,
    to: Option<Key>,
    cache: impl Into<Option<&'a mut cache::Sqlite>>,
) -> anyhow::Result<DiffResult> {
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
                    if y[x] {
                        // directories don't have a hash
                        None
                    } else {
                        // this is a file
                        match dir::hash_fs_item(
                            ds,
                            x,
                            *cache.expect("you must pass a cache if you're hashing the fs"),
                        ) {
                            Ok(h) => Some(h),
                            Err(e) => panic!("{}", e),
                        }
                    }
                },
                |y| Some(y[x].0),
            ),
            is_dir: from_map.as_ref().either(|y| y[x], |y| y[x].1),
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
                )?;
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

pub fn print_stat_diff_result(ds: &impl DataStore, r: DiffResult) {
    let stat = line_stat(ds, r);

    print_line_stat(stat);
}

pub fn print_patch_diff_result(ds: &impl DataStore, r: DiffResult) -> anyhow::Result<()> {
    println!("{}", create_diff_patch_result(ds, r)?);

    Ok(())
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
    tracing::debug!("{:?}", &r);

    let mut items = Vec::new();

    for added in r.added.into_iter().filter(|x| !x.is_dir) {
        if let Some(k) = added.new_key {
            items.push(FileStatResult {
                fname: added.path,
                added: line_ct(ds, k),
                removed: 0,
            });
        }
    }

    for removed in r.deleted.into_iter().filter(|x| !x.is_dir) {
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
        file::read_data(ds, modified.original_key, &mut before).unwrap();

        let mut after = Vec::new();
        file::read_data(ds, modified.new_key, &mut after).unwrap();

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

pub fn line_ct(ds: &impl DataStore, key: Key) -> usize {
    let mut data = Vec::new();
    file::read_data(ds, key, &mut data).unwrap();

    #[allow(clippy::naive_bytecount)]
    // This whole function will be cached in the store at some point, this is just for testing
    data.iter().filter(|x| **x == b'\n').count()
}

#[must_use]
pub fn format_patch(p: &patch::Patch<'_>) -> String {
    format!("{}\n", p)
}

fn print_str_diff(old: &str, new: &str, old_name: &str, new_name: &str, to: &mut String) {
    use std::fmt::Write;

    use similar::TextDiff;

    write!(
        to,
        "{}",
        TextDiff::from_lines(old, new)
            .unified_diff()
            .header(old_name, new_name)
    )
    .expect("writing to a string to never fail");
}

pub fn create_diff_patch_result(ds: &impl DataStore, r: DiffResult) -> anyhow::Result<String> {
    let mut result = String::new();

    tracing::debug!("{:?}", &r);

    for added in r.added {
        if added.is_dir {
        } else if let Some(k) = added.new_key {
            let path = added.path.to_string_lossy();

            let mut data = Vec::new();
            file::read_data(ds, k, &mut data).unwrap();

            let data = std::str::from_utf8(&data);

            match data {
                Ok(s) => print_str_diff("", s, "/dev/null", &path, &mut result),
                Err(_) => anyhow::bail!("this is a binary file"),
            }
        }
    }

    for removed in r.deleted {
        if removed.is_dir {
        } else if let Some(k) = removed.original_key {
            let path = removed.path.to_string_lossy();

            let mut data = Vec::new();
            file::read_data(ds, k, &mut data).unwrap();

            let data = std::str::from_utf8(&data);

            match data {
                Ok(s) => print_str_diff(s, "", &path, "/dev/null", &mut result),
                Err(_) => anyhow::bail!("this is a binary file"),
            }
        }
    }

    for modified in r.modified {
        let path = modified.path.to_string_lossy();
        let mut before = Vec::new();

        tracing::debug!("{:?}", &modified);

        file::read_data(ds, modified.original_key, &mut before).unwrap();

        let mut after = Vec::new();
        file::read_data(ds, modified.new_key, &mut after).unwrap();

        let before_str = String::from_utf8_lossy(&before);
        let after_str = String::from_utf8_lossy(&after);

        print_str_diff(&before_str, &after_str, &path, &path, &mut result);
    }

    Ok(result)
}
