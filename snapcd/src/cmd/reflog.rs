use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};
use crate::cmd::common::{State, CmdResult, DatabaseNotFoundError};
use crate::{
    cache::SqliteCache, commit, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};
use crate::cmd::common::NoHeadError;
use colored::*;

#[derive(StructOpt, Debug)]
pub enum RefCommand {
    Log(RefLogArgs),
    Update(RefUpdateArgs),
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

pub fn ref_cmd(state: &mut State, args: RefCommand) -> CmdResult {
    match args {
        RefCommand::Log(args) => ref_log(state, args),
        RefCommand::Update(args) => ref_update(state, args),
    }
}

fn ref_log(state: &mut State, args: RefLogArgs) -> CmdResult {
    let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

    let refname = match args.refname {
        Some(s) => s,
        None => ds_state.ds.get_head()?.ok_or(NoHeadError)?,
    };

    let keys = ds_state.ds.reflog_walk(&refname, args.remote.as_deref())?;

    println!(
        "{}",
        "log entries are printed with most recent at top".bright_black()
    );

    for (idx, key) in keys.iter().enumerate() {
        println!("{}: {}", keys.len() - idx, key);
    }

    Ok(())
}

fn ref_update(state: &mut State, args: RefUpdateArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let refname = match args.refname {
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

