use colored::*;

use crate::{commit, file, object, DataStore, key::Key, diff};
use std::convert::TryInto;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShowError {
    #[error("error reading data from db: {_0}")]
    DataReadError(#[from] file::ReadDataError),
}

pub fn display_obj(ds: &mut impl DataStore, key: Key) -> Result<(), ShowError> {
    let obj = ds.get_obj(key).unwrap().into_owned();

    use object::ObjType;

    match obj.objtype() {
        ObjType::FileBlobTree | ObjType::FileBlob => {
            file::read_data(ds, key, &mut std::io::stdout())?;
        }
        ObjType::FSItemFile => {
            assert!(obj.keys().len() == 1);
            display_obj(ds, obj.keys()[0])?;
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
            let parent_obj = ds.get_obj(parent.unwrap().inner()).unwrap();

            let parent_cmt: commit::Commit = parent_obj
                .into_owned()
                .try_into()
                .expect("failed to convert commit obj");


            let dr = diff::compare(ds, diff::DiffTarget::Database(commit_obj.tree().into()), Some(parent_cmt.tree().into()), None).unwrap();

            diff::print_patch_diff_result(ds, dr);
        }
        ObjType::FSItemDir => todo!(),
        ObjType::Unknown => {
            panic!("cannot display unknown object");
        }
    }

    Ok(())
}
