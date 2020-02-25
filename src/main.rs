// StructOpt generated code triggers this lint.
#![allow(clippy::option_unwrap_used)]
#![allow(clippy::result_unwrap_used)]
// I don't care.
#![allow(clippy::needless_pass_by_value)]

use snapcd::{
    cache::SqliteCache, commit, diff, dir, display, ds::sqlite::SqliteDS, ds::GetReflogError,
    ds::Transactional, filter, DataStore, Keyish, Reflog,
};

pub use thiserror::Error;
use colored::*;

use std::path::{Path, PathBuf};
use structopt::StructOpt;

type CMDResult = Result<(), anyhow::Error>;

use structopt::clap::AppSettings;
use std::convert::TryInto;

#[derive(StructOpt, Debug)]
#[structopt(global_setting=AppSettings::ColoredHelp)]
struct Opt {
    #[structopt(flatten)]
    common: Common,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug)]
struct Common {
    /// Path to database folder
    #[structopt(short = "-d", long = "--db", default_value = ".snapcd")]
    db_path: PathBuf,

    /// Verbosity. Provide multiple times to increase (-vv, -vvv).
    #[structopt(short = "-v", parse(from_occurrences), global = true)]
    verbosity: u64,

    #[structopt(short = "-q", long = "--quiet", global = true)]
    quiet: bool,

    /// Paths to exclude
    #[structopt(short = "-e", long = "--exclude", number_of_values(1), global = true)]
    exclude: Vec<String>,
}

struct State {
    ds_state: Option<DsState>,
    cache: SqliteCache,
    common: Common,
}

struct DsState {
    ds: SqliteDS,
    db_folder_path: PathBuf,
    repo_path: PathBuf,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// Inserts a file into the database and prints its hash.
    Insert(InsertArgs),

    /// Commits a file
    Commit(CommitArgs),

    /// Fetches a file from the database by hash
    Fetch(FetchArgs),

    /// Debugging tools
    Debug(DebugCommand),

    /// Initialises the database
    Init(InitArgs),

    /// Shows an object
    Show(ShowArgs),

    /// Compares a path with an object tree
    Compare(CompareArgs),

    /// Gets status
    Status(StatusArgs),

    /// Checks out
    Checkout(CheckoutArgs),

    CheckoutHead(CheckoutHeadArgs),

    Ref(RefCommand),
}

#[derive(StructOpt, Debug)]
enum RefCommand {
    Log(RefLogArgs),
    Update(RefUpdateArgs),
}

#[derive(StructOpt, Debug)]
struct RefLogArgs {
    refname: Option<String>,
    remote: Option<String>,
}

#[derive(StructOpt, Debug)]
struct RefUpdateArgs {
    key: Keyish,
    refname: Option<String>,
}

#[derive(StructOpt, Debug)]
struct CheckoutHeadArgs {
    refname: String,
}

#[derive(StructOpt, Debug)]
struct CheckoutArgs {}

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
struct CommitArgs {
    #[structopt(short, long)]
    path: Option<PathBuf>,

    #[structopt(short, long)]
    message: String,

    refname: Option<String>,
}

#[derive(StructOpt, Debug)]
struct ShowArgs {
    /// Object to show
    key: Keyish,
}

#[derive(StructOpt, Debug)]
struct InsertArgs {
    /// Path of the file to insert
    path: PathBuf,
}

#[derive(StructOpt, Debug)]
struct FetchArgs {
    /// Hex-encoded hash (As output by 'insert')
    key: Keyish,

    /// Destination path to write to
    dest: PathBuf,
}

#[derive(StructOpt, Debug)]
enum DebugCommand {
    PrettyPrint(PrettyPrintArgs),
    CommitTree(CommitTreeArgs),
    ReflogGet(ReflogGetArgs),
    ReflogPush(ReflogPushArgs),
    WalkTree(WalkTreeArgs),
    WalkFsTree(WalkFsTreeArgs),
    SetHead(SetHeadArgs),
    GetHead(GetHeadArgs),
}

#[derive(StructOpt, Debug)]
pub struct GetHeadArgs {}

#[derive(StructOpt, Debug)]
pub struct SetHeadArgs {
    refname: String,
}

#[derive(StructOpt, Debug)]
pub struct WalkTreeArgs {
    key: Keyish,
}

#[derive(StructOpt, Debug)]
struct WalkFsTreeArgs {
    path: PathBuf,
}

#[derive(StructOpt, Debug)]
struct PrettyPrintArgs {
    key: Keyish,
}

#[derive(StructOpt, Debug)]
struct InitArgs {}

#[derive(StructOpt, Debug)]
struct ReflogGetArgs {
    refname: String,
    remote: Option<String>,
}

#[derive(StructOpt, Debug)]
struct ReflogPushArgs {
    key: Keyish,
    refname: String,
    remote: Option<String>,
}

#[derive(StructOpt, Debug)]
struct CommitTreeArgs {
    tree: Keyish,
    parents: Vec<Keyish>,
}

#[derive(Debug, Error)]
#[error("database could not be found (maybe run snapcd init)")]
struct DatabaseNotFoundError;

#[derive(Debug, Error)]
#[error(
    "an operation that requires a HEAD was run, without being given one, and no head has been set"
)]
struct NoHeadError;

fn insert(state: &mut State, args: InsertArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let filter = filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

    let hash = dir::put_fs_item(&mut ds_state.ds, &args.path, &filter)?;

    println!("inserted hash {}", hash);

    Ok(())
}

fn fetch(state: &mut State, args: FetchArgs) -> CMDResult {
    let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    dir::get_fs_item(&ds_state.ds, key.into(), &args.dest)?;

    Ok(())
}

fn debug(state: &mut State, args: DebugCommand) -> CMDResult {
    match args {
        DebugCommand::PrettyPrint(args) => debug_pretty_print(state, args),
        DebugCommand::CommitTree(args) => debug_commit_tree(state, args),
        DebugCommand::ReflogGet(args) => debug_reflog_get(state, args),
        DebugCommand::ReflogPush(args) => debug_reflog_push(state, args),
        DebugCommand::WalkTree(args) => debug_walk_tree(state, args),
        DebugCommand::WalkFsTree(args) => debug_walk_fs_tree(state, args),
        DebugCommand::SetHead(args) => debug_set_head(state, args),
        DebugCommand::GetHead(args) => debug_get_head(state, args),
    }
}

fn ref_log(state: &mut State, args: RefLogArgs) -> CMDResult {
    let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

    let refname = match args.refname {
        Some(s) => s,
        None => ds_state.ds.get_head()?.ok_or(NoHeadError)?,
    };

    let keys = ds_state
        .ds
        .reflog_walk(&refname, args.remote.as_deref())?;

    println!("{}", "log entries are printed with most recent at top".bright_black());

    for (idx, key) in keys.iter().enumerate() {
        println!("{}: {}", keys.len()-idx, key);
    }

    Ok(())
}

fn ref_update(state: &mut State, args: RefUpdateArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let refname = match args.refname {
        Some(s) => s,
        None => ds_state.ds.get_head()?.ok_or(NoHeadError)?,
    };

    let log = Reflog {
        key: key.into(),
        refname,
        remote: None,
    };

    ds_state.ds.reflog_push(&log)?;

    Ok(())
}

fn ref_cmd(state: &mut State, args: RefCommand) -> CMDResult {
    match args {
        RefCommand::Log(args) => ref_log(state, args),
        RefCommand::Update(args) => ref_update(state, args),
    }
}

fn debug_set_head(state: &mut State, args: SetHeadArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    ds_state.ds.put_head(&args.refname)?;

    Ok(())
}

fn debug_get_head(state: &mut State, _args: GetHeadArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let head = ds_state.ds.get_head()?;
    println!("head: {:?}", head);

    Ok(())
}

fn debug_walk_tree(state: &mut State, args: WalkTreeArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let fs_items = dir::walk_fs_items(&ds_state.ds, key.into())?;

    for item in fs_items {
        println!("{:?}", item)
    }

    Ok(())
}

fn debug_walk_fs_tree(_state: &mut State, args: WalkFsTreeArgs) -> CMDResult {
    let fs_items = dir::walk_real_fs_items(&args.path, &|_| true)?;

    for item in fs_items {
        println!("{:?}, {}", item.0, item.1)
    }

    Ok(())
}

fn debug_pretty_print(state: &mut State, args: PrettyPrintArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let item = ds_state.ds.get_obj(key)?;

    item.debug_pretty_print();

    Ok(())
}

fn debug_commit_tree(state: &mut State, args: CommitTreeArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let tree = ds_state.ds.canonicalize(args.tree)?;

    let mut parents = Vec::with_capacity(args.parents.len());

    for parent in args.parents {
        let key = ds_state.ds.canonicalize(parent)?;
        parents.push(key);
    }

    let attrs = commit::CommitAttrs::default();

    let commit = commit::commit_tree(&mut ds_state.ds, tree.into(), parents.iter().map(|&x| x.into()).collect(), attrs)?;

    println!("{}", commit);

    Ok(())
}

fn debug_reflog_get(state: &mut State, args: ReflogGetArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state
        .ds
        .reflog_get(&args.refname, args.remote.as_deref())?;

    println!("{}", key);

    Ok(())
}

fn debug_reflog_push(state: &mut State, args: ReflogPushArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let log = Reflog {
        key: key.into(),
        refname: args.refname,
        remote: args.remote,
    };

    ds_state.ds.reflog_push(&log)?;

    Ok(())
}

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

fn init(state: &mut State, _args: InitArgs) -> CMDResult {
    std::fs::create_dir_all(&state.common.db_path)?;
    let ds = SqliteDS::new(&state.common.db_path.join("snapcd.db"))?;

    ds.put_head("master")?;

    Ok(())
}

fn commit_cmd(state: &mut State, args: CommitArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let filter = filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

    let commit_path = match &args.path {
        Some(p) => p,
        None => &ds_state.repo_path,
    };

    let refname = match args.refname {
        Some(name) => name,
        None => ds_state.ds.get_head()?.ok_or(NoHeadError)?,
    };

    let try_got_key = ds_state.ds.reflog_get(&refname, None);

    let parent_key = match try_got_key {
        Ok(k) => vec![k],
        Err(GetReflogError::NotFound) => vec![],
        Err(other) => return Err(other.into()),
    };

    let key = dir::put_fs_item(&mut ds_state.ds, &commit_path, &filter)?;

    let mut attrs = commit::CommitAttrs::default();

    attrs.set_message(args.message);

    let commit_key = commit::commit_tree(&mut ds_state.ds, key.into(), parent_key, attrs)?;

    let log = Reflog {
        key: commit_key,
        refname,
        remote: None,
    };

    ds_state.ds.reflog_push(&log)?;

    Ok(())
}

fn show(state: &mut State, args: ShowArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    display::display_obj(&mut ds_state.ds, key)?;

    Ok(())
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

fn compare(state: &mut State, args: CompareArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = match args.key {
        Some(k) => ds_state.ds.canonicalize(k)?.into(),
        None => {
            let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;
            let key = ds_state.ds.reflog_get(&reflog, None)?;
            commit::Commit::from_key(&ds_state.ds, key).tree()
        }
    };

    let path = match &args.path {
        Some(p) => p,
        None => &ds_state.repo_path,
    };

    

    let result = diff::compare(
        &mut ds_state.ds,
        diff::DiffTarget::FileSystem(
            path.clone(),
            state.common.exclude.clone(),
            ds_state.db_folder_path.clone(),
        ),
        Some(key),
        &mut state.cache,
    )?;

    if args.stat {
        diff::print_stat_diff_result(&ds_state.ds, result);
    } else {
        diff::print_diff_result(result);
    }

    Ok(())
}

fn status(state: &mut State, _args: StatusArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;

    let path = &ds_state.repo_path;

    let ref_key = ds_state.ds.reflog_get(&reflog, None).ok();

    match &ref_key {
        Some(k) => {
            println!("HEAD: {} [{}]", reflog, &k.inner().as_user_key()[0..8]);
        }
        None => {
            println!("HEAD: {} (no commits on {})", reflog, reflog);
        }
    }

    let obj: commit::Commit = ds_state.ds.get_obj(ref_key.unwrap().into()).unwrap().into_owned().try_into().unwrap();

    let result = diff::compare(
        &mut ds_state.ds,
        diff::DiffTarget::FileSystem(
            path.to_path_buf(),
            state.common.exclude.clone(),
            ds_state.db_folder_path.clone(),
        ),
        Some(obj.tree()),
        &mut state.cache,
    )?;

    diff::print_diff_result(result);

    Ok(())
}

fn checkout(state: &mut State, _args: CheckoutArgs) -> CMDResult {
    let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

    let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;
    let key = ds_state.ds.reflog_get(&reflog, None)?;

    let filter = filter::make_filter_fn(&state.common.exclude, ds_state.db_folder_path.clone());

    let tree_key = commit::Commit::from_key(&ds_state.ds, key).tree();
    dir::checkout_fs_item(&ds_state.ds, tree_key, &ds_state.repo_path, &filter)?;
    Ok(())
}

fn checkout_head(state: &mut State, args: CheckoutHeadArgs) -> CMDResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let reflog = ds_state.ds.get_head()?.ok_or(NoHeadError)?;

    let path = &ds_state.repo_path;

    let ref_key = ds_state.ds.reflog_get(&reflog, None).ok();

    let tree_key = ref_key.map(|key| commit::Commit::from_key(&ds_state.ds, key).tree());

    let result = diff::compare(
        &mut ds_state.ds,
        diff::DiffTarget::FileSystem(
            path.to_path_buf(),
            state.common.exclude.clone(),
            ds_state.db_folder_path.clone(),
        ),
        tree_key,
        &mut state.cache,
    )?;

    if !diff::diff_result_empty(&result) {
        println!("Cannot checkout: working directory is not clean");

        diff::print_diff_result(result);
        return Ok(());
    }

    ds_state.ds.put_head(&args.refname)?;

    println!("Ok, new head.");

    Ok(())
}

fn setup_logging(#[allow(unused_variables)] level: u64) {
    #[cfg(feature = "logging")]
    {
        use simplelog::{LevelFilter, TermLogError, TerminalMode};

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
            Err(TermLogError::SetLogger(_)) => {
                panic!("logger has been already set, this is a bug.")
            }
            Err(TermLogError::Term) => eprintln!("failed to open terminal for logging"),
            // how are we printing this then?
        }
    }
}

fn main() -> CMDResult {
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

            let ds = SqliteDS::new(x.join("snapcd.db"))?;

            Some(DsState {
                db_folder_path,
                ds,
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
        Command::Insert(args) => insert(&mut state, args),
        Command::Fetch(args) => fetch(&mut state, args),
        Command::Debug(args) => debug(&mut state, args),
        Command::Init(args) => init(&mut state, args),
        Command::Commit(args) => commit_cmd(&mut state, args),
        Command::Show(args) => show(&mut state, args),
        Command::Compare(args) => compare(&mut state, args),
        Command::Status(args) => status(&mut state, args),
        Command::Checkout(args) => checkout(&mut state, args),
        Command::CheckoutHead(args) => checkout_head(&mut state, args),
        Command::Ref(args) => ref_cmd(&mut state, args),
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
