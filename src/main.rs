// StructOpt generated code triggers this lint.
#![allow(clippy::option_unwrap_used)]
#![allow(clippy::result_unwrap_used)]
// I don't care.
#![allow(clippy::needless_pass_by_value)]

use snapcd::{dir, DataStore, Keyish, SqliteDS};
use std::path::PathBuf;
use structopt::StructOpt;

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
}

#[derive(StructOpt, Debug)]
struct PrettyPrintArgs {
    key: Keyish,
}

fn insert(state: &mut State, args: InsertArgs) -> CMDResult {
    let hash = dir::put_fs_item(&mut state.ds, &args.path)?;

    println!("{}", hash);

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
    }
}

fn debug_pretty_print(state: &mut State, args: PrettyPrintArgs) -> CMDResult {
    let key = state.ds.canonicalize(args.key)?;

    let item = state.ds.get_obj(&key)?;

    println!("{}", item);

    Ok(())
}

fn main() -> CMDResult {
    let opt = Opt::from_args();

    let mut ds = SqliteDS::new(&opt.common.db_path)?;

    ds.begin_trans()?;

    let mut state = State { ds };

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
