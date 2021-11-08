use libsnapcd::ds::{sqlite::Sqlite, DataStore};
use structopt::StructOpt;

use crate::cmd::{CmdResult, CommandTrait, State};

#[derive(StructOpt, Debug)]
pub struct InitArgs {}

impl CommandTrait for InitArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        std::fs::create_dir_all(&state.common.db_path)?;
        let ds = Sqlite::new(&state.common.db_path.join("snapcd.db"))?;

        ds.put_head("main")?;

        Ok(())
    }
}
