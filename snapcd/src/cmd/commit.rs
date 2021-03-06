use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};
use crate::cmd::common::{State, CmdResult, DatabaseNotFoundError};
use crate::{
    cache::SqliteCache, commit, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};

pub fn commit_cmd(state: &mut State, args: CommitArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let filter = filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

    let commit_path = match &args.path {
        Some(p) => p,
        None => &ds_state.repo_path,
    };

    let refname = match args.refname {
        Some(name) => name,
        None => ds_state.ds.get_head()?.ok_or(crate::cmd::common::NoHeadError)?,
    };

    let try_got_key = ds_state.ds.reflog_get(&refname, None);

    let parent_key = match try_got_key {
        Ok(k) => vec![k],
        Err(GetReflogError::NotFound) => vec![],
        Err(other) => return Err(other.into()),
    };

    let key = dir::put_fs_item(&mut ds_state.ds, &commit_path, &filter)?;

    let attrs = crate::object::CommitAttrs {
        message: args.message,
        ..Default::default()
    };

    let commit_key = commit::commit_tree(&mut ds_state.ds, key, parent_key, attrs)?;

    let log = Reflog {
        key: commit_key,
        refname,
        remote: None,
    };

    ds_state.ds.reflog_push(&log)?;

    Ok(())
}

#[derive(StructOpt, Debug)]
pub struct CommitArgs {
    #[structopt(short, long)]
    path: Option<PathBuf>,

    #[structopt(short, long)]
    message: String,

    refname: Option<String>,
}

