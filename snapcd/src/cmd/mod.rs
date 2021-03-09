// from_occurrences causes this
#![allow(trivial_numeric_casts)]

pub mod checkout;
pub mod commit;
pub mod debug;
pub mod fetch;
pub mod init;
pub mod insert;
pub mod reflog;
pub mod status;

use structopt::{clap::AppSettings, StructOpt};
use thiserror::Error;

use crate::cache::SqliteCache;
use crate::ds::sqlite::SqliteDs;
use std::path::PathBuf;

#[enum_dispatch::enum_dispatch]
pub trait CommandTrait {
    fn execute(self, state: &mut State) -> CmdResult;
}

#[derive(Debug, Error)]
#[error("database could not be found (maybe run snapcd init)")]
pub struct DatabaseNotFoundError;

#[derive(Debug, Error)]
#[error(
    "an operation that requires a HEAD was run, without being given one, and no head has been set"
)]
pub struct NoHeadError;

pub type CmdResult = Result<(), anyhow::Error>;

#[derive(StructOpt, Debug)]
#[structopt(global_setting=AppSettings::ColoredHelp)]
pub struct Opt {
    #[structopt(flatten)]
    pub common: Common,
    #[structopt(subcommand)]
    pub cmd: Command,
}

#[derive(StructOpt, Debug)]
pub struct Common {
    /// Path to database folder
    #[structopt(short = "-d", long = "--db", default_value = ".snapcd")]
    pub db_path: PathBuf,

    /// Verbosity. Provide multiple times to increase (-vv, -vvv).
    #[structopt(short = "-v", parse(from_occurrences), global = true)]
    pub verbosity: u64,

    #[structopt(short = "-q", long = "--quiet", global = true)]
    pub quiet: bool,

    /// Paths to exclude
    #[structopt(short = "-e", long = "--exclude", number_of_values(1), global = true)]
    pub exclude: Vec<String>,
}

#[enum_dispatch::enum_dispatch(CommandTrait)]
#[derive(StructOpt, Debug)]
pub enum Command {
    /// Inserts a file into the database and prints its hash.
    Insert(insert::InsertArgs),

    /// Commits a file
    Commit(commit::CommitArgs),

    /// Fetches a file from the database by hash
    Fetch(fetch::FetchArgs),

    /// Debugging tools
    Debug(debug::DebugCommand),

    /// Initialises the database
    Init(init::InitArgs),

    /// Checks out
    Checkout(checkout::CheckoutArgs),

    Ref(reflog::RefCommand),

    Status(status::StatusArgs),
}

#[derive(Debug)]
pub struct State {
    pub ds_state: Option<DsState>,
    pub cache: SqliteCache,
    pub common: Common,
}

#[derive(Debug)]
pub struct DsState {
    pub ds: SqliteDs,
    pub db_folder_path: PathBuf,
    pub repo_path: PathBuf,
}
