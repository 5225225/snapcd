// StructOpt generated code triggers this lint.
#![allow(clippy::option_unwrap_used)]
#![allow(clippy::result_unwrap_used)]
// I don't care.
#![allow(clippy::needless_pass_by_value)]

use failure::Fallible;
use snapcd::{commit, dir, DataStore, Keyish, Reflog, SqliteDS};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

use slog;
use slog::{o, Drain};

type CMDResult = Fallible<()>;

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(flatten)]
    common: Common,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug)]
struct Common {
    /// Path to sqlite database
    #[structopt(short = "-d", long = "--db", default_value = "snapcd.db")]
    db_path: PathBuf,
}

struct State {
    ds: Option<SqliteDS>,
    #[allow(dead_code)]
    logger: slog::Logger,
    common: Common,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// Inserts a file into the database and prints its hash.
    Insert(InsertArgs),

    /// Fetches a file from the database by hash
    Fetch(FetchArgs),

    /// Debugging tools
    Debug(DebugCommand),

    /// Initialises the database
    Init(InitArgs),
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

fn insert(state: &mut State, args: InsertArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let hash = dir::put_fs_item(ds, &args.path)?;

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
    }
}

fn debug_pretty_print(state: &mut State, args: PrettyPrintArgs) -> CMDResult {
    let ds = state.ds.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds.canonicalize(args.key)?;

    let item = ds.get_obj(&key)?;

    println!("{}", item);

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

fn find_db_file(name: &Path) -> Fallible<Option<PathBuf>> {
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

fn init(state: &mut State, args: InitArgs) -> CMDResult {
    SqliteDS::new(&state.common.db_path)?;

    Ok(())
}

fn main() -> CMDResult {
    let opt = Opt::from_args();

    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());

    let logger = slog::Logger::root(slog_term::FullFormat::new(plain).build().fuse(), o!());

    let ds: Option<SqliteDS> = match find_db_file(&opt.common.db_path) {
        Ok(Some(x)) => Some(SqliteDS::new(x)?),
        Ok(None) => None,
        Err(x) => return Err(x),
    };

    let mut state = State {
        ds,
        logger,
        common: opt.common,
    };

    state.ds.as_mut().map(|x| x.begin_trans());

    let result = match opt.cmd {
        Command::Insert(args) => insert(&mut state, args),
        Command::Fetch(args) => fetch(&mut state, args),
        Command::Debug(args) => debug(&mut state, args),
        Command::Init(args) => init(&mut state, args),
    };

    if let Err(e) = result {
        println!("fatal: {:?}", e);

        state.ds.as_mut().map(|x| x.rollback());
    } else {
        state.ds.as_mut().map(|x| x.commit());
    }

    Ok(())
}
