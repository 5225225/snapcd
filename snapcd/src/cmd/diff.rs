use libsnapcd::{diff, ds::DataStore};
use structopt::StructOpt;

use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, NoHeadError, State};

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

        let ref_key = ds_state.ds.reflog_get(&reflog, None).ok();

        match &ref_key {
            Some(k) => {
                let from = if let Some(k) = self.from {
                    ds_state.ds.canonicalize(k)?
                } else {
                    panic!("oh no");
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

                let result = diff::compare(&mut ds_state.ds, from, to)?;

                diff::print_diff_result(result);
            }
            None => {
                println!("HEAD: {} (no commits on {})", reflog, reflog);
            }
        }

        Ok(())
    }
}
