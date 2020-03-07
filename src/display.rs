use colored::*;

use crate::{commit, diff, file, key::Key, object, DataStore};
use std::convert::TryInto;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShowError {
    #[error("error reading data from db: {_0}")]
    DataReadError(#[from] file::ReadDataError),

    #[error("error showing diff patch: {_0}")]
    DiffPatch(#[from] diff::DiffPatchError),
}

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Stat,
    Patch,
}

pub fn log_obj(ds: &mut impl DataStore, key: Key, kind: Kind) -> Result<(), ShowError> {
    let obj = ds.get_obj(key).unwrap().into_owned();

    match obj.objtype() {
        object::ObjType::Commit => {
            display_obj(ds, key, kind)?;

            println!();
            println!();

            let mut obj = ds.get_obj(key).unwrap().into_owned();

            let mut commit_obj: commit::Commit = obj
                .into_owned()
                .try_into()
                .expect("failed to convert commit obj");

            while let Some(p) = commit_obj.parents().get(0).copied() {
                display_obj(ds, p.into(), kind)?;

                println!();
                println!();

                obj = ds.get_obj(p.inner()).unwrap().into_owned();

                commit_obj = obj
                    .into_owned()
                    .try_into()
                    .expect("failed to convert commit obj");
            }
        }
        _ => panic!("Invalid key type"),
    }

    Ok(())
}

pub fn display_obj(ds: &mut impl DataStore, key: Key, kind: Kind) -> Result<(), ShowError> {
    let obj = ds.get_obj(key).unwrap().into_owned();

    use object::ObjType;

    match obj.objtype() {
        ObjType::FileBlobTree | ObjType::FileBlob => {
            file::read_data(ds, key, &mut std::io::stdout())?;
        }
        ObjType::FSItemFile => {
            assert!(obj.keys().len() == 1);
            display_obj(ds, obj.keys()[0], kind)?;
        }
        ObjType::Commit => {
            println!("{}", format!("commit: {}", key).yellow());

            let commit_obj: commit::Commit = obj
                .into_owned()
                .try_into()
                .expect("failed to convert commit obj");

            println!();

            println!("{}", commit_obj.attrs().message());

            println!();

            let parent = commit_obj.parents().get(0).copied();

            let tree;
            if let Some(p) = parent {
                let parent_obj = ds.get_obj(p.inner()).unwrap();

                let parent_cmt: commit::Commit = parent_obj
                    .into_owned()
                    .try_into()
                    .expect("failed to convert commit obj");

                tree = Some(parent_cmt.tree());
            } else {
                tree = None;
            }

            let dr = diff::compare(
                ds,
                diff::DiffTarget::Database(commit_obj.tree()),
                tree,
                None,
            )
            .unwrap();

            match kind {
                Kind::Stat => {
                    diff::print_stat_diff_result(ds, dr);
                }
                Kind::Patch => {
                    diff::print_patch_diff_result(ds, dr)?;
                }
            }
        }
        ObjType::FSItemDir => {
            println!("{}", format!("tree: {}", key).yellow());
        }
        ObjType::Unknown => {
            panic!("cannot display unknown object");
        }
    }

    Ok(())
}
