use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, NoHeadError, State};
use crate::diff::DiffTarget;
use crate::object::Object;
use crate::{diff, DataStore};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct LogArgs {}

impl CommandTrait for LogArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;

        let head_key = ds_state.ds.reflog_get(&reflog, None).ok();

        match &head_key {
            Some(mut k) => loop {
                let (tree, parents) = if let Object::Commit {
                    tree,
                    parents,
                    attrs: _,
                } = ds_state.ds.get_obj(k).unwrap()
                {
                    (tree, parents)
                } else {
                    panic!("unexpected object type")
                };

                if parents.is_empty() {
                    break;
                }

                let parent = ds_state.ds.get_obj(parents[0])?.tree(parents[0]).unwrap();

                let result = diff::compare(
                    &mut ds_state.ds,
                    DiffTarget::Database(tree),
                    Some(parent),
                    &mut state.cache,
                )?;

                diff::print_diff_result(result);
                k = parents[0];
            },
            None => {
                println!("HEAD: {} (no commits on {})", reflog, reflog);
            }
        }

        Ok(())
    }
}
