use crate::cmd::{CmdResult, CommandTrait, State, DatabaseNotFoundError, NoHeadError};
use crate::{ds::sqlite::SqliteDs, DataStore, diff};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct StatusArgs {}

impl CommandTrait for StatusArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;

        let path = &ds_state.repo_path;

        let ref_key = ds_state.ds.reflog_get(&reflog, None).ok();

        match &ref_key {
            Some(k) => {
                println!("HEAD: {} [{}]", reflog, &k.as_user_key()[0..8]);

                /*
                let obj: = ds_state
                    .ds
                    .get_obj(k)
                    .unwrap()
                    .into_owned()
                    .try_into()
                    .unwrap();

                let result = diff::compare(
                    &mut ds_state.ds,
                    diff::DiffTarget::FileSystem(
                        path.to_path_buf(),
                        state.common.exclude.clone(),
                        ds_state.db_folder_path.clone(),
                    ),
                    Some(obj.tree()),
                    &mut state.cache,
                )?;

                diff::print_diff_result(result);
                */
            }
            None => {
                println!("HEAD: {} (no commits on {})", reflog, reflog);
            }
        }

        Ok(())
    }
}
