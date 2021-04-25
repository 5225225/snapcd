use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, State};
use crate::entry::Entry;
use crate::{dir, filter};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InsertArgs {
    /// Path of the file to insert
    path: PathBuf,
}

impl CommandTrait for InsertArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let entry = Entry::from_path(&self.path, cap_std::ambient_authority());

        let filter = filter::include_all;

        let hash = dir::put_fs_item(&mut ds_state.ds, &entry, "".into(), &filter)?;

        println!("inserted hash {}", hash);

        Ok(())
    }
}
