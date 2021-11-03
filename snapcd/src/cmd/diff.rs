use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, NoHeadError, State};
use libsnapcd::{diff, ds::DataStore};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct DiffArgs {
    #[structopt(short, long)]
    from: Option<libsnapcd::keyish::Keyish>,

    #[structopt(short, long)]
    to: Option<libsnapcd::keyish::Keyish>,
}

impl CommandTrait for DiffArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;

        let path = &ds_state.repo_path;

        let ref_key = ds_state.ds.reflog_get(&reflog, None).ok();

        match &ref_key {
            Some(k) => {
                let from = if let Some(k) = self.from {
                    diff::DiffTarget::Database(ds_state.ds.canonicalize(k)?)
                } else {
                    diff::DiffTarget::FileSystem(
                        path.to_path_buf(),
                        state.common.exclude.clone(),
                        ds_state.db_folder_path.clone(),
                    )
                };

                let to = if let Some(k) = self.to {
                    let key = ds_state.ds.canonicalize(k)?;
                    ds_state
                        .ds
                        .get_obj(key)
                        .unwrap()
                        .tree(key)
                        .expect("can't do tree on this kind of object")
                } else {
                    ds_state
                        .ds
                        .get_obj(*k)
                        .unwrap()
                        .tree(*k)
                        .expect("can't do tree on this kind of object")
                };

                let result = diff::compare(&mut ds_state.ds, from, Some(to), &mut state.cache)?;

                diff::print_diff_result(result);
            }
            None => {
                println!("HEAD: {} (no commits on {})", reflog, reflog);
            }
        }

        Ok(())
    }
}
