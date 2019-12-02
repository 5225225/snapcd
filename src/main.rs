use std::path::PathBuf;
use structopt::StructOpt;
use std::str::FromStr;
use snapcd::{SqliteDS, DataStore, KeyBuf};

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(flatten)]
    common: Common,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug)]
struct Common {
    db_path: String,
}

struct State {
    ds: SqliteDS,
}

#[derive(StructOpt, Debug)]
enum Command {
    Insert(InsertArgs),
    Fetch(FetchArgs),
}

#[derive(StructOpt, Debug)]
struct InsertArgs {
    path: PathBuf,
}

#[derive(StructOpt, Debug)]
struct FetchArgs {
    key: String,
    dest: PathBuf,
}


fn insert(mut state: State, args: InsertArgs) {
    let f = std::fs::File::open(args.path).unwrap();

    let reader = std::io::BufReader::new(f);

    let hash = state.ds.put_data(reader);

    println!("{}", hash);
}

fn fetch(mut state: State, args: FetchArgs) {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.dest)
        .unwrap();

    let key = KeyBuf::from_str(&args.key).unwrap();

    state.ds.read_data(key.as_key(), &mut f);
}


fn main() {
    let opt = Opt::from_args();

    let ds = SqliteDS::new(&opt.common.db_path);

    let state = State {
        ds
    };

    match opt.cmd {
        Command::Insert(args) => insert(state, args),
        Command::Fetch(args) => fetch(state, args),
    }
}
