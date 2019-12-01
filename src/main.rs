use std::path::PathBuf;
use structopt::StructOpt;
use snapcd::{SqliteDS, DataStore};

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
}

#[derive(StructOpt, Debug)]
struct InsertArgs {
    path: PathBuf,
}


fn insert(mut state: State, args: InsertArgs) {
    let f = std::fs::File::open(args.path).unwrap();

    let hash = state.ds.put_data(f);

    println!("{}", hash);
}


fn main() {
    let opt = Opt::from_args();

    let ds = SqliteDS::new(&opt.common.db_path);

    let state = State {
        ds
    };

    match opt.cmd {
        Command::Insert(args) => insert(state, args),
    }
}
