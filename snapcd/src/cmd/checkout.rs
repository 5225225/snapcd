use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};
use crate::cmd::common::{State, CmdResult, DatabaseNotFoundError, NoHeadError};
use crate::{
    cache::SqliteCache, commit, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};

#[derive(StructOpt, Debug)]
pub struct CheckoutArgs {}

pub fn checkout(state: &mut State, _args: CheckoutArgs) -> CmdResult {
    let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

    let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;
    let key = ds_state.ds.reflog_get(&reflog, None)?;

    let filter = filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

    let tree_key = match ds_state.ds.get_obj(key)? {
        Object::Commit { tree, .. } => tree,
        _ => panic!("invalid reflog value"),
    };

    dir::checkout_fs_item(&ds_state.ds, tree_key, &ds_state.repo_path, &filter)?;
    Ok(())
}
