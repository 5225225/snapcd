use snapcd::cmd::CommandTrait;
use snapcd::cmd::{DsState, Opt, State};
use snapcd::logging::{setup_logging, setup_sqlite_callback};
use libsnapcd::{cache::SqliteCache, ds::sqlite::SqliteDs};
use structopt::StructOpt;

type CmdResult = Result<(), anyhow::Error>;

fn main() -> CmdResult {
    let opt = Opt::from_args();

    setup_logging(opt.common.verbosity);

    // This is safe because we won't run this in parallel with any SQLite commands.
    unsafe {
        setup_sqlite_callback();
    }

    tracing::debug!("parsed command line: {:?}", opt);

    let ds_state: Option<DsState> = match libsnapcd::ds::find_db_folder(&opt.common.db_path) {
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

    tracing::info!(
        "using db folder path {:?}",
        ds_state.as_ref().map(|x| &x.db_folder_path)
    );
    tracing::info!(
        "using repo path {:?}",
        ds_state.as_ref().map(|x| &x.repo_path)
    );

    let cache = match dirs::cache_dir() {
        Some(mut d) => {
            tracing::info!("using cache dir {}", d.display());
            d.push("snapcd");
            std::fs::create_dir_all(&d)?;
            d.push("cache.db");
            SqliteCache::new(d)?
        }
        None => {
            tracing::warn!("cache not found, using in memory cache");
            SqliteCache::new(":memory:")?
        }
    };

    let mut state = State {
        ds_state,
        cache,
        common: opt.common,
    };

    let result = opt.cmd.execute(&mut state);

    if let Err(e) = result {
        tracing::debug!("error debug: {:?}", e);

        println!("fatal: {}", e);
    }

    Ok(())
}
