use snapcd::{file, DataStore, KeyBuf, SqliteDS, dir};
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;

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
    db_path: String,
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
    key: String,

    /// Destination path to write to
    dest: PathBuf,
}

#[derive(StructOpt, Debug)]
enum DebugCommand {
    PrettyPrint(PrettyPrintArgs),
}

#[derive(StructOpt, Debug)]
struct PrettyPrintArgs {
    key: String,
}

fn insert(mut state: State, args: InsertArgs) {
    let hash = dir::put_fs_item(&mut state.ds, &args.path);

    println!("{}", hash);
}

#[allow(clippy::needless_pass_by_value)]
fn fetch(state: State, args: FetchArgs) {
    let key = KeyBuf::from_str(&args.key).unwrap();

    dir::get_fs_item(&state.ds, key.as_key(), &args.dest);
}

fn debug(state: State, args: DebugCommand) {
    match args {
        DebugCommand::PrettyPrint(args) => debug_pretty_print(state, args),
    }
}

#[allow(clippy::needless_pass_by_value)]
fn debug_pretty_print(state: State, args: PrettyPrintArgs) {
    let key = state
        .ds
        .get_obj(KeyBuf::from_str(&args.key).unwrap().as_key());

    println!("{}", key);
}

fn main() {
    let opt = Opt::from_args();

    let ds = SqliteDS::new(&opt.common.db_path);

    let state = State { ds };

    match opt.cmd {
        Command::Insert(args) => insert(state, args),
        Command::Fetch(args) => fetch(state, args),
        Command::Debug(args) => debug(state, args),
    }
}
