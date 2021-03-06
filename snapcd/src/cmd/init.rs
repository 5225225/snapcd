use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};
use crate::cmd::common::{State, CmdResult, DatabaseNotFoundError};
use crate::{
    cache::SqliteCache, commit, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};

pub fn init(state: &mut State, _args: InitArgs) -> CmdResult {
    std::fs::create_dir_all(&state.common.db_path)?;
    let ds = SqliteDs::new(&state.common.db_path.join("snapcd.db"))?;

    ds.put_head("master")?;

    Ok(())
}


#[derive(StructOpt, Debug)]
pub struct InitArgs {}
