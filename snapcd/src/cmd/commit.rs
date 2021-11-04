use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, NoHeadError, State};

use libsnapcd::{ds::DataStore, filter};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct CommitArgs {
    #[structopt(short, long)]
    path: Option<PathBuf>,

    #[structopt(short, long)]
    message: String,

    refname: Option<String>,
}
impl CommandTrait for CommitArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let _filter =
            filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

        let _commit_path = match &self.path {
            Some(p) => p,
            None => &ds_state.repo_path,
        };

        let refname = match self.refname {
            Some(name) => name,
            None => ds_state.ds.get_head()?.ok_or(NoHeadError)?,
        };

        let _try_got_key = ds_state.ds.reflog_get(&refname, None);

        // TODO: reflog_get needs to handle a not found
        /*
        let parent_key = match try_got_key {
            Ok(k) => vec![k],
            Err(GetReflogError::NotFound) => vec![],
            Err(other) => return Err(other.into()),
        };

        let entry = Entry::from_path(commit_path, cap_std::ambient_authority());

        let key = dir::put_fs_item(&mut ds_state.ds, &entry, "".into(), &filter)?;

        let attrs = libsnapcd::object::CommitAttrs {
            message: self.message,
            ..Default::default()
        };

        let commit_key = commit::commit_tree(&mut ds_state.ds, key, parent_key, attrs)?;

        let log = Reflog {
            key: commit_key,
            refname,
            remote: None,
        };

        ds_state.ds.reflog_push(&log)?;

        */

        Ok(())
    }
}
