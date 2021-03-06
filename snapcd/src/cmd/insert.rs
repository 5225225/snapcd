use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, State};
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

        let filter = filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

        let hash = dir::put_fs_item(&mut ds_state.ds, &self.path, &filter)?;

        println!("inserted hash {}", hash);

        Ok(())
    }
}
