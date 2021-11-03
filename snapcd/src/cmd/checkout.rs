use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, NoHeadError, State};
use libsnapcd::{dir, ds::DataStore, filter, object::Object};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct CheckoutArgs {}

impl CommandTrait for CheckoutArgs {
    fn execute(self, state: &mut State) -> CmdResult {
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
}
