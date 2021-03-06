use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};
use crate::cmd::common::{State, CmdResult, DatabaseNotFoundError};
use crate::{
    cache::SqliteCache, commit, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};

#[derive(StructOpt, Debug)]
pub struct InsertArgs {
    /// Path of the file to insert
    path: PathBuf,
}


pub fn insert(state: &mut State, args: InsertArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let filter = filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

    let hash = dir::put_fs_item(&mut ds_state.ds, &args.path, &filter)?;

    println!("inserted hash {}", hash);

    Ok(())
}
