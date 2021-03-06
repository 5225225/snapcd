use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};

use crate::ds::sqlite::SqliteDs;
use crate::cache::SqliteCache;
use thiserror::Error;

use super::{insert, commit, fetch, debug, reflog, checkout, init};

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
}

pub struct State {
    pub ds_state: Option<DsState>,
    pub cache: SqliteCache,
    pub common: Common,
}

pub struct DsState {
    pub ds: SqliteDs,
    pub db_folder_path: PathBuf,
    pub repo_path: PathBuf,
}

