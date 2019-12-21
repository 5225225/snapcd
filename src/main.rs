// StructOpt generated code triggers this lint.
#![allow(clippy::option_unwrap_used)]
#![allow(clippy::result_unwrap_used)]
// I don't care.
#![allow(clippy::needless_pass_by_value)]

use snapcd::{dir, DataStore, Keyish, SqliteDS, commit};
use std::path::PathBuf;
use structopt::StructOpt;
use std::collections::HashMap;

use slog;
use slog::{Drain, o};

type CMDResult = failure::Fallible<()>;

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
    ds: SqliteDS,
    #[allow(dead_code)]
    logger: slog::Logger,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// Inserts a file into the database and prints its hash.
    Insert(InsertArgs),

    /// Fetches a file from the database by hash
    Fetch(FetchArgs),

    /// Debugging tools
    Debug(DebugCommand),
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
}

#[derive(StructOpt, Debug)]
struct PrettyPrintArgs {
    key: Keyish,
}

#[derive(StructOpt, Debug)]
struct CommitTreeArgs {
    tree: Keyish,
    parents: Vec<Keyish>,
}

fn insert(state: &mut State, args: InsertArgs) -> CMDResult {
    let hash = dir::put_fs_item(&mut state.ds, &args.path)?;

    println!("inserted hash {}", hash);

    Ok(())
}

fn fetch(state: &mut State, args: FetchArgs) -> CMDResult {
    let key = state.ds.canonicalize(args.key)?;

    dir::get_fs_item(&state.ds, &key, &args.dest)?;

    Ok(())
}

fn debug(state: &mut State, args: DebugCommand) -> CMDResult {
    match args {
        DebugCommand::PrettyPrint(args) => debug_pretty_print(state, args),
        DebugCommand::CommitTree(args) => debug_commit_tree(state, args),
    }
}

fn debug_pretty_print(state: &mut State, args: PrettyPrintArgs) -> CMDResult {
    let key = state.ds.canonicalize(args.key)?;

    let item = state.ds.get_obj(&key)?;

    println!("{}", item);

    Ok(())
}

fn debug_commit_tree(state: &mut State, args: CommitTreeArgs) -> CMDResult {
    let tree = state.ds.canonicalize(args.tree)?;

    let mut parents = Vec::with_capacity(args.parents.len());

    for parent in args.parents {
        let key = state.ds.canonicalize(parent)?;
        parents.push(key);
    }

    let attrs = HashMap::new();

    let commit = commit::commit_tree(&mut state.ds, tree, parents, attrs)?;

    println!("{}", commit);

    Ok(())
}

fn main() -> CMDResult {
    let opt = Opt::from_args();

    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());

    let logger = slog::Logger::root(
        slog_term::FullFormat::new(plain).build().fuse(),
        o!()
    );

    let mut ds = SqliteDS::new(&opt.common.db_path)?;

    ds.begin_trans()?;

    let mut state = State { ds, logger };

    let result = match opt.cmd {
        Command::Insert(args) => insert(&mut state, args),
        Command::Fetch(args) => fetch(&mut state, args),
        Command::Debug(args) => debug(&mut state, args),
    };

    if let Err(e) = result {
        println!("fatal: {:?}", e);

        state.ds.rollback()?;
    } else {
        state.ds.commit()?;
    }

    Ok(())
}
