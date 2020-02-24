use colored::*;

use crate::{commit, file, object, DataStore, Key};
use std::convert::TryInto;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShowError {
    #[error("error reading data from db: {_0}")]
    DataReadError(#[from] file::ReadDataError),
}

pub fn display_obj(ds: &impl DataStore, key: Key) -> Result<(), ShowError> {
    let obj = ds.get_obj(key).unwrap();

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
        }
        ObjType::FSItemDir => todo!(),
        ObjType::Unknown => {
            panic!("cannot display unknown object");
        }
    }

    Ok(())
}
