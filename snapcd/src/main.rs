// StructOpt generated code triggers this lint.
#![allow(clippy::unwrap_used)]
// I don't care.
#![allow(clippy::needless_pass_by_value)]

use snapcd::{
    cache::SqliteCache, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};

use snapcd::cmd::common::{State, Command, DsState, Opt};
use snapcd::cmd::{insert, fetch, debug, init, commit, checkout, reflog};

use colored::*;
pub use thiserror::Error;

use std::path::{Path, PathBuf};
use structopt::StructOpt;

type CmdResult = Result<(), anyhow::Error>;

use structopt::clap::AppSettings;

#[derive(StructOpt, Debug)]
struct CheckoutHeadArgs {
    refname: String,
}

#[derive(StructOpt, Debug)]
struct StatusArgs {}

#[derive(StructOpt, Debug)]
struct CompareArgs {
    #[structopt(short = "-p", long = "--path")]
    path: Option<PathBuf>,
    key: Option<Keyish>,

    #[structopt(short = "-s", long = "--stat")]
    stat: bool,
}

#[derive(StructOpt, Debug)]
struct ShowArgs {
    /// Object to show
    key: Option<Keyish>,
}

#[derive(StructOpt, Debug)]
struct LogArgs {
    /// Object to show
    key: Option<Keyish>,
}

#[derive(Debug, Error)]
#[error("database could not be found (maybe run snapcd init)")]
struct DatabaseNotFoundError;

fn find_db_folder(name: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
    let cwd = std::env::current_dir()?;

    let mut d = &*cwd;

    loop {
        let mut check = d.to_path_buf();

        check.push(&name);

        if check.exists() {
            return Ok(Some(check));
        }

        d = match d.parent() {
            Some(p) => p,
            None => return Ok(None),
        };
    }
}

fn sqlite_logging_callback(err_code: i32, err_msg: &str) {
    log::warn!("sqlite error {}: {}", err_code, err_msg);
}

fn setup_sqlite_callback() -> rusqlite::Result<()> {
    unsafe {
        // This is unsafe because it is not thread safe ("No other SQLite calls may be made while
        // config_log is running, and multiple threads may not call config_log simultaneously.")
        // as well sqlite_logging_callback having the requirements that they do not invoke SQLite,
        // and must be thread safe itself.
        rusqlite::trace::config_log(Some(sqlite_logging_callback))?;
    }

    Ok(())
}

fn setup_logging(#[allow(unused_variables)] level: u64) {
    #[cfg(feature = "logging")]
    {
        use simplelog::{LevelFilter, TerminalMode};

        let filter = match level {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            3..=std::u64::MAX => LevelFilter::Trace,
        };

        let log_config = simplelog::ConfigBuilder::new()
            .set_time_level(LevelFilter::Debug)
            .set_time_to_local(true)
            .build();

        match simplelog::TermLogger::init(filter, log_config, TerminalMode::Stderr) {
            Ok(()) => {}
            Err(err) => {
                panic!("{}: logger has been already set, this is a bug.", err)
            }
        }
    }
}

fn main() -> CmdResult {
    let opt = Opt::from_args();

    setup_logging(opt.common.verbosity);

    setup_sqlite_callback()?;

    log::debug!("parsed command line: {:?}", opt);

    let ds_state: Option<DsState> = match find_db_folder(&opt.common.db_path) {
        Ok(Some(x)) => {
            let db_folder_path = x.clone();
            let repo_path = x
                .parent()
                .expect("failed to get parent of db folder?")
                .into();

            let ds = SqliteDs::new(x.join("snapcd.db"))?;

            Some(DsState {
                ds,
                db_folder_path,
                repo_path,
            })
        }
        Ok(None) => None,
        Err(x) => return Err(x),
    };

    log::info!(
        "using db folder path {:?}",
        ds_state.as_ref().map(|x| &x.db_folder_path)
    );
    log::info!(
        "using repo path {:?}",
        ds_state.as_ref().map(|x| &x.repo_path)
    );

    let cache = match dirs::cache_dir() {
        Some(mut d) => {
            log::info!("using cache dir {}", d.display());
            d.push("snapcd");
            std::fs::create_dir_all(&d)?;
            d.push("cache.db");
            SqliteCache::new(d)?
        }
        None => {
            log::warn!("cache not found, using in memory cache");
            SqliteCache::new(":memory:")?
        }
    };

    let mut state = State {
        ds_state,
        cache,
        common: opt.common,
    };

    state.ds_state.as_mut().map(|x| x.ds.begin_trans());
    state.cache.begin_trans()?;

    let result = match opt.cmd {
        Command::Insert(args) => insert::insert(&mut state, args),
        Command::Fetch(args) => fetch::fetch(&mut state, args),
        Command::Debug(args) => debug::debug(&mut state, args),
        Command::Init(args) => init::init(&mut state, args),
        Command::Commit(args) => commit::commit_cmd(&mut state, args),
        Command::Checkout(args) => checkout::checkout(&mut state, args),
        Command::Ref(args) => reflog::ref_cmd(&mut state, args),
    };

    if let Err(e) = result {
        log::debug!("error debug: {:?}", e);

        println!("fatal: {}", e);

        state.ds_state.as_mut().map(|x| x.ds.rollback());
        state.cache.rollback()?;
    } else {
        state.ds_state.as_mut().map(|x| x.ds.commit());
        state.cache.commit()?;
    }

    Ok(())
}
