use crate::key::Key;
use crate::DataStore;
use crate::{cache, dir, file, filter};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use thiserror::Error;

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

pub fn compare<'a>(
    ds: &'a mut impl DataStore,
    from: DiffTarget,
    to: Option<Key>,
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
            log::debug!("{:?}", fs_items);
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
                            Ok(h) => Some(h),
                            Err(e) => panic!("{}", e),
                        }
                    } else {
                        // directories don't have a hash
                        None
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

            log::debug!("diff: pushing {:?} to modified because {} != {}", dr, f, t.0);

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

pub fn print_patch_diff_result(ds: &impl DataStore, r: DiffResult) -> Result<(), DiffPatchError> {
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
    ldbg!(&r);

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

pub fn diff_result_empty(r: &DiffResult) -> bool {
    r.added.is_empty() && r.deleted.is_empty() && r.modified.is_empty()
}

pub fn format_patch(p: &patch::Patch<'_>) -> String {
    format!("{}\n", p)
}

const BEFORE_CONTEXT_LINES: usize = 3;
const AFTER_CONTEXT_LINES: usize = 5;

#[derive(Debug, Error)]
pub enum DiffPatchError {
    #[error("tried to make patch with non-UTF8 files")]
    Binary,
}

pub fn create_diff_patch_result(
    ds: &impl DataStore,
    r: DiffResult,
) -> Result<String, DiffPatchError> {
    let mut result = String::new();

    ldbg!(&r);

    for added in r.added {
        if added.is_dir {
        } else if let Some(k) = added.new_key {
            let path = added.path.to_string_lossy();
            let meta = None;

            let old = patch::File {
                path: path.clone(),
                meta: meta.clone(),
            };
            let new = patch::File { path, meta }; // For now we're just saying old == new

            let mut hunks = Vec::new();

            let mut data = Vec::new();
            file::read_data(ds, k, &mut data).unwrap();

            let data = std::str::from_utf8(&data);

            match data {
                Ok(s) => {
                    let mut lines = Vec::new();

                    for line in s.lines() {
                        lines.push(patch::Line::Add(line));
                    }

                    hunks.push(patch::Hunk {
                        old_range: patch::Range { start: 0, count: 0 },
                        new_range: patch::Range {
                            start: 0,
                            count: lines.len() as u64,
                        },
                        lines,
                    });
                }
                Err(_) => {
                    return Err(DiffPatchError::Binary);
                }
            }

            let patch = patch::Patch {
                old,
                new,
                hunks,
                end_newline: true,
            };

            let formatted_patch = format_patch(&patch);

            result.push_str(&formatted_patch);
        }
    }

    for removed in r.deleted {
        if removed.is_dir {
        } else if let Some(k) = removed.original_key {
            let path = removed.path.to_string_lossy();
            let meta = None;

            let old = patch::File {
                path: path.clone(),
                meta: meta.clone(),
            };
            let new = patch::File { path, meta }; // For now we're just saying old == new

            let mut hunks = Vec::new();
            let mut data = Vec::new();
            file::read_data(ds, k, &mut data).unwrap();

            let data = std::str::from_utf8(&data);

            match data {
                Ok(s) => {
                    let mut lines = Vec::new();

                    for line in s.lines() {
                        lines.push(patch::Line::Remove(line));
                    }

                    hunks.push(patch::Hunk {
                        new_range: patch::Range { start: 0, count: 0 },
                        old_range: patch::Range {
                            start: 0,
                            count: lines.len() as u64,
                        },
                        lines,
                    });
                }
                Err(_) => {
                    return Err(DiffPatchError::Binary);
                }
            }

            let patch = patch::Patch {
                old,
                new,
                hunks,
                end_newline: true,
            };

            let formatted_patch = format_patch(&patch);

            result.push_str(&formatted_patch);
        }
    }

    for modified in r.modified {
        let mut before = Vec::new();

        ldbg!(&modified);

        file::read_data(ds, modified.original_key, &mut before).unwrap();

        let mut after = Vec::new();
        file::read_data(ds, modified.new_key, &mut after).unwrap();

        let before_str = String::from_utf8_lossy(&before);
        let after_str = String::from_utf8_lossy(&after);

        let patch_str =
            patch_from_file_string(before_str.to_string(), after_str.to_string(), modified.path);

        result.push_str(&patch_str);
    }

    Ok(result)
}

fn patch_from_file_string(before_str: String, after_str: String, path: PathBuf) -> String {
    let lines = difference::Changeset::new(&before_str, &after_str, "\n");

    let mut before_lineno = 1;
    let mut after_lineno = 1;

    let lines_ln: Vec<_> = lines
        .diffs
        .into_iter()
        .map(|x| {
            let mut result = Vec::new();
            match x {
                difference::Difference::Add(s) => {
                    for line in s.split('\n') {
                        result.push((
                            before_lineno,
                            after_lineno,
                            difference::Difference::Add(line.to_string()),
                        ));
                        after_lineno += 1;
                    }
                }
                difference::Difference::Same(s) => {
                    for line in s.split('\n') {
                        result.push((
                            before_lineno,
                            after_lineno,
                            difference::Difference::Same(line.to_string()),
                        ));
                        before_lineno += 1;
                        after_lineno += 1;
                    }
                }
                difference::Difference::Rem(s) => {
                    for line in s.split('\n') {
                        result.push((
                            before_lineno,
                            after_lineno,
                            difference::Difference::Rem(line.to_string()),
                        ));
                        before_lineno += 1;
                    }
                }
            }
            result
        })
        .flatten()
        .collect();

    let mut windows_vec = Vec::new();
    for (idx, item) in lines_ln.iter().enumerate() {
        let context = &lines_ln[(idx.saturating_sub(BEFORE_CONTEXT_LINES))
            ..=(idx
                .saturating_add(AFTER_CONTEXT_LINES)
                .min(lines_ln.len() - 1))];

        windows_vec.push((item, context));
    }

    let groups = windows_vec.into_iter().group_by(|x| {
        x.1.iter()
            .any(|y| !matches!(y.2, difference::Difference::Same(_)))
    });

    let mut hunks = Vec::new();

    for (key, group) in &groups {
        if !key {
            continue;
        }

        let collected: Vec<_> = group.map(|x| x.0).collect();

        assert!(!collected.is_empty());

        let (start_before, start_after, _) = collected.first().unwrap();
        let (end_before, end_after, _) = collected.last().unwrap();

        let before_range = patch::Range {
            start: *start_before,
            count: *end_before - *start_before,
        };

        let after_range = patch::Range {
            start: *start_after,
            count: *end_after - *start_after,
        };

        let mut lines = Vec::new();

        for item in collected.iter().map(|x| &x.2) {
            let patch_item = match item {
                difference::Difference::Same(x) => patch::Line::Context(&x),
                difference::Difference::Add(x) => patch::Line::Add(&x),
                difference::Difference::Rem(x) => patch::Line::Remove(&x),
            };

            lines.push(patch_item);
        }

        let hunk = patch::Hunk {
            old_range: before_range,
            new_range: after_range,
            lines,
        };

        hunks.push(hunk);
    }

    let patch = patch::Patch {
        old: patch::File {
            path: path.to_string_lossy(),
            meta: None,
        },
        new: patch::File {
            path: path.to_string_lossy(),
            meta: None,
        },
        hunks,
        end_newline: true,
    };

    format_patch(&patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn patch_from_file_string_doesnt_panic(before: String, after: String, path: String) {
            let _ = patch_from_file_string(before, after, path.into());
        }
    }
}
