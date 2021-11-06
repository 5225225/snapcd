use std::path::PathBuf;

use libsnapcd::{dir, ds::DataStore, keyish::Keyish};
use structopt::StructOpt;

use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, State};

#[derive(StructOpt, Debug)]
pub struct FetchArgs {
    /// Hex-encoded hash (As output by 'insert')
    key: Keyish,

    /// Destination path to write to
    dest: PathBuf,
}

impl CommandTrait for FetchArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

        let key = ds_state.ds.canonicalize(self.key)?;

        //        let entry = Entry::from_path(&self.dest, cap_std::ambient_authority());

        dir::get_fs_item(&ds_state.ds, key, &self.dest, cap_std::ambient_authority())?;

        Ok(())
    }
}
