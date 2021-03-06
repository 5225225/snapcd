use crate::cmd::{CmdResult, CommandTrait, State};
use crate::{ds::sqlite::SqliteDs, DataStore};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InitArgs {}

impl CommandTrait for InitArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        std::fs::create_dir_all(&state.common.db_path)?;
        let ds = SqliteDs::new(&state.common.db_path.join("snapcd.db"))?;

        ds.put_head("main")?;

        Ok(())
    }
}
