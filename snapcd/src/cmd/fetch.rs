use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};
use crate::cmd::common::{State, CmdResult, DatabaseNotFoundError};
use crate::{
    cache::SqliteCache, commit, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};


pub fn fetch(state: &mut State, args: FetchArgs) -> CmdResult {
    let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    dir::get_fs_item(&ds_state.ds, key, &args.dest)?;

    Ok(())
}

#[derive(StructOpt, Debug)]
pub struct FetchArgs {
    /// Hex-encoded hash (As output by 'insert')
    key: Keyish,

    /// Destination path to write to
    dest: PathBuf,
}

