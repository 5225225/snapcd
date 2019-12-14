// StructOpt generated code triggers this lint.
#![allow(clippy::option_unwrap_used)]
#![allow(clippy::result_unwrap_used)]

use snapcd::{DataStore, KeyBuf, SqliteDS, dir, Keyish};
use std::path::PathBuf;
use std::str::FromStr;
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

#[allow(clippy::needless_pass_by_value)]
fn insert(mut state: State, args: InsertArgs) -> CMDResult {
    let hash = dir::put_fs_item(&mut state.ds, &args.path)?;

    println!("{}", hash);

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn fetch(state: State, args: FetchArgs) -> CMDResult {
    let key = state.ds.canonicalize(args.key)?;

    dir::get_fs_item(&state.ds, key.as_key(), &args.dest)?;

    Ok(())
}

fn debug(state: State, args: DebugCommand) -> CMDResult {
    match args {
        DebugCommand::PrettyPrint(args) => debug_pretty_print(state, args),
    }
}

#[allow(clippy::needless_pass_by_value)]
fn debug_pretty_print(state: State, args: PrettyPrintArgs) -> CMDResult {
    let key = state.ds.canonicalize(args.key)?;

    let item = state
        .ds
        .get_obj(key.as_key())?;

    println!("{}", item);

    Ok(())
}

fn main() -> CMDResult {
    let opt = Opt::from_args();

    let ds = SqliteDS::new(&opt.common.db_path)?;

    let state = State { ds };

    let result = match opt.cmd {
        Command::Insert(args) => insert(state, args),
        Command::Fetch(args) => fetch(state, args),
        Command::Debug(args) => debug(state, args),
    };

    if let Err(e) = result {
        println!("fatal: {}", e);
    }

    Ok(())
}
