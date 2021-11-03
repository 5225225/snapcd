use crate::cmd::NoHeadError;
use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, State};
use libsnapcd::{ds::DataStore, keyish::Keyish, ds::Reflog};
use colored::*;
use structopt::StructOpt;

#[enum_dispatch::enum_dispatch]
pub trait RefCommandTrait {
    fn execute(self, state: &mut State) -> CmdResult;
}

#[enum_dispatch::enum_dispatch(RefCommandTrait)]
#[derive(StructOpt, Debug)]
pub enum RefCommand {
    Log(RefLogArgs),
    Update(RefUpdateArgs),
}

impl CommandTrait for RefCommand {
    fn execute(self, state: &mut State) -> CmdResult {
        RefCommandTrait::execute(self, state)
    }
}

#[derive(StructOpt, Debug)]
pub struct RefLogArgs {
    refname: Option<String>,
    remote: Option<String>,
}

#[derive(StructOpt, Debug)]
pub struct RefUpdateArgs {
    key: Keyish,
    refname: Option<String>,
}

impl RefCommandTrait for RefLogArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

        let refname = match self.refname {
            Some(s) => s,
            None => ds_state.ds.get_head()?.ok_or(NoHeadError)?,
        };

        let keys = ds_state.ds.reflog_walk(&refname, self.remote.as_deref())?;

        println!(
            "{}",
            "log entries are printed with most recent at top".bright_black()
        );

        for (idx, key) in keys.iter().enumerate() {
            println!("{}: {}", keys.len() - idx, key);
        }

        Ok(())
    }
}

impl RefCommandTrait for RefUpdateArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let key = ds_state.ds.canonicalize(self.key)?;

        let refname = match self.refname {
            Some(s) => s,
            None => ds_state.ds.get_head()?.ok_or(NoHeadError)?,
        };

        let log = Reflog {
            key,
            refname,
            remote: None,
        };

        ds_state.ds.reflog_push(&log)?;

        Ok(())
    }
}
