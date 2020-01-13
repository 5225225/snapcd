// StructOpt generated code triggers this lint.
#![allow(clippy::option_unwrap_used)]
#![allow(clippy::result_unwrap_used)]
// I don't care.
#![allow(clippy::needless_pass_by_value)]

use failure::Fallible;
use snapcd::{
    cache::{Cache, SqliteCache},
    commit, dir, DataStore, Keyish, Reflog, SqliteDS,
};
use std::collections::{HashMap, HashSet};
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

use simplelog::{LevelFilter, TermLogError, TerminalMode};

type CMDResult = Fallible<()>;

use structopt::clap::AppSettings;

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
    #[structopt(
        short = "-d",
        long = "--db",
        default_value = ".snapcd",
    )]
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
    ds: Option<SqliteDS>,
    db_folder_path: Option<PathBuf>,
    cache: SqliteCache,
    common: Common,
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
}

#[derive(StructOpt, Debug)]
struct CompareArgs {
    path: PathBuf,
    key: Keyish,
}

#[derive(StructOpt, Debug)]
struct CommitArgs {

    path: PathBuf,
    refname: String,
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

#[derive(Debug, failure_derive::Fail)]
#[fail(display = "database could not be found (maybe run snapcd init)")]
struct DatabaseNotFoundError;

fn make_filter_fn<T: AsRef<str>>(excludes: &[T], db_path: &Option<PathBuf>) -> Box<dyn Fn(&DirEntry) -> bool> {
    let mut excl_globs = globset::GlobSetBuilder::new();

    for exclude in excludes {
        excl_globs.add(globset::Glob::new(exclude.as_ref()).unwrap());
    }

    let excl_globset = excl_globs.build().unwrap();

    let cloned_db_path: Option<PathBuf> = db_path.clone();

    Box::new(move |direntry: &DirEntry| -> bool {
        let path = direntry.path();

        if let Some(p) = &cloned_db_path {
            let canon_path = std::fs::canonicalize(&path).unwrap();
            let canon_p = std::fs::canonicalize(&p).unwrap();

            if canon_path.starts_with(canon_p) {
                return false;
            }
        }

        let normalised_path;

        if path.starts_with("./") {
            normalised_path = path.strip_prefix("./").unwrap();
        } else {
            normalised_path = &path;
        }

        !excl_globset.is_match(normalised_path)
    })
}

fn insert(state: &mut State, args: InsertArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let filter = make_filter_fn(&state.common.exclude, &state.db_folder_path);

    let hash = dir::put_fs_item(ds, &args.path, &filter)?;

    println!("inserted hash {}", hash);

    Ok(())
}

fn fetch(state: &mut State, args: FetchArgs) -> CMDResult {
    let ds = state.ds.as_ref().ok_or(DatabaseNotFoundError)?;

    let key = ds.canonicalize(args.key)?;

    dir::get_fs_item(ds, &key, &args.dest)?;

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
    }
}

fn debug_walk_tree(state: &mut State, args: WalkTreeArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds.canonicalize(args.key)?;

    let fs_items = dir::walk_fs_items(ds, &key)?;

    for item in fs_items {
        println!("{:?}", item)
    }

    Ok(())
}

fn debug_walk_fs_tree(state: &mut State, args: WalkFsTreeArgs) -> CMDResult {
    let fs_items = dir::walk_real_fs_items(&args.path, &|_| true)?;

    for item in fs_items {
        println!("{:?}, {}", item.0, item.1)
    }

    Ok(())
}

fn debug_pretty_print(state: &mut State, args: PrettyPrintArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds.canonicalize(args.key)?;

    let item = ds.get_obj(&key)?;

    println!("{}", item.debug_pretty_print());

    Ok(())
}

fn debug_commit_tree(state: &mut State, args: CommitTreeArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let tree = ds.canonicalize(args.tree)?;

    let mut parents = Vec::with_capacity(args.parents.len());

    for parent in args.parents {
        let key = ds.canonicalize(parent)?;
        parents.push(key);
    }

    let attrs = HashMap::new();

    let commit = commit::commit_tree(ds, tree, parents, attrs)?;

    println!("{}", commit);

    Ok(())
}

fn debug_reflog_get(state: &mut State, args: ReflogGetArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds.reflog_get(&args.refname, args.remote.as_deref())?;

    println!("{}", key);

    Ok(())
}

fn debug_reflog_push(state: &mut State, args: ReflogPushArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds.canonicalize(args.key)?;

    let log = Reflog {
        key,
        refname: args.refname,
        remote: args.remote,
    };

    ds.reflog_push(&log)?;

    Ok(())
}

fn find_db_folder(name: &Path) -> Fallible<Option<PathBuf>> {
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
    SqliteDS::new(&state.common.db_path.join("snapcd.db"))?;

    Ok(())
}

fn commit_cmd(state: &mut State, args: CommitArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let filter = make_filter_fn(&state.common.exclude, &state.db_folder_path);

    let key = dir::put_fs_item(ds, &args.path, &filter)?;

    let log = Reflog {
        key,
        refname: args.refname,
        remote: None,
    };

    ds.reflog_push(&log)?;

    Ok(())
}

fn show(state: &mut State, args: ShowArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds.canonicalize(args.key)?;

    let value = ds.get_obj(&key)?;

    println!("{}", value.show());

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
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds.canonicalize(args.key)?;

    let db_items = dir::walk_fs_items(ds, &key)?;
    let db_items_keys: HashSet<_> = db_items.keys().collect();

    let exclude = make_filter_fn(&state.common.exclude, &state.db_folder_path);

    let fs_items = dir::walk_real_fs_items(&args.path, &exclude)?;
    let fs_items_keys: HashSet<_> = fs_items.keys().collect();

    let in_db_only = db_items_keys.difference(&fs_items_keys);
    let in_fs_only = fs_items_keys.difference(&db_items_keys);
    let in_both = fs_items_keys.intersection(&db_items_keys);

    for item in in_db_only {
        println!("deleted: {}", item.display());
    }

    for item in in_fs_only {
        println!("added:   {}", item.display());
    }

    for item in in_both {
        let db_key = &db_items[*item];

        if fs_items[*item] {
            continue;
        }

        let fs_item_key = dir::hash_fs_item(ds, &args.path.join(item), &state.cache)?;

        if db_key.0 != fs_item_key {
            println!("modified: {}", item.display());
        }
    }

    Ok(())
}

fn main() -> CMDResult {
    let opt = Opt::from_args();

    let log_config = simplelog::ConfigBuilder::new()
        .set_time_level(LevelFilter::Debug)
        .set_time_to_local(true)
        .build();

    let filter = match opt.common.verbosity {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        3..=std::u64::MAX => LevelFilter::Trace,
    };

    match simplelog::TermLogger::init(filter, log_config, TerminalMode::Stderr) {
        Ok(()) => {}
        Err(TermLogError::SetLogger(_)) => panic!("logger has been already set, this is a bug."),
        Err(TermLogError::Term) => eprintln!("failed to open terminal for logging"),
        // how are we printing this then?
    }

    setup_sqlite_callback()?;

    log::debug!("parsed command line: {:?}", opt);

    let db_folder_path;

    let ds: Option<SqliteDS> = match find_db_folder(&opt.common.db_path) {
        Ok(Some(x)) => {
            log::info!("using db path {}", x.display());
            db_folder_path = Some(x.clone());
            Some(SqliteDS::new(x.join("snapcd.db"))?)
        }
        Ok(None) => {
            log::info!("found no db");
            db_folder_path = None;
            None
        }
        Err(x) => return Err(x),
    };


    let cache = match dirs::cache_dir() {
        Some(mut d) => {
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
        ds,
        cache,
        common: opt.common,
        db_folder_path,
    };

    state.ds.as_mut().map(|x| x.begin_trans());
    state.cache.begin_trans();

    let result = match opt.cmd {
        Command::Insert(args) => insert(&mut state, args),
        Command::Fetch(args) => fetch(&mut state, args),
        Command::Debug(args) => debug(&mut state, args),
        Command::Init(args) => init(&mut state, args),
        Command::Commit(args) => commit_cmd(&mut state, args),
        Command::Show(args) => show(&mut state, args),
        Command::Compare(args) => compare(&mut state, args),
    };

    if let Err(e) = result {
        log::debug!("error debug: {:?}", e);

        println!("fatal: {}", e);

        state.ds.as_mut().map(|x| x.rollback());
        state.cache.rollback();
    } else {
        state.ds.as_mut().map(|x| x.commit());
        state.cache.commit();
    }

    Ok(())
}
