use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, State};
use crate::DataStore;
use structopt::StructOpt;

#[enum_dispatch::enum_dispatch]
pub trait NetCommandTrait {
    fn execute(self, state: &mut State) -> CmdResult;
}

#[enum_dispatch::enum_dispatch(NetCommandTrait)]
#[derive(StructOpt, Debug)]
pub enum NetCommand {
    Get(GetCommand),
    Put(PutCommand),
}

impl CommandTrait for NetCommand {
    fn execute(self, state: &mut State) -> CmdResult {
        NetCommandTrait::execute(self, state)
    }
}

#[derive(StructOpt, Debug)]
pub struct GetCommand {
    key: crate::key::Key,

    #[structopt(short, long)]
    recursive: bool,
}

#[derive(StructOpt, Debug)]
pub struct PutCommand {
    key: crate::keyish::Keyish,

    #[structopt(short, long)]
    recursive: bool,
}

impl NetCommandTrait for GetCommand {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

        let conn = crate::network::Connection {
            url: "http://localhost:8000".to_string(),
        };

        get(&conn, &ds_state.ds, self.key, self.recursive);

        Ok(())
    }
}

fn get(conn: &crate::network::Connection, state: &dyn DataStore, key: crate::key::Key, recursive: bool) {
    let obj = conn.get(key);

    state.put_obj(&obj).unwrap();

    if recursive {
        for key in obj.links() {
            get(conn, state, key, true);
        }
    }
}

impl NetCommandTrait for PutCommand {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

        let conn = crate::network::Connection {
            url: "http://localhost:8000".to_string(),
        };

        let k = ds_state.ds.canonicalize(self.key).unwrap();

        put(&conn, &ds_state.ds, k, self.recursive);

        Ok(())
    }
}

fn put(conn: &crate::network::Connection, state: &dyn DataStore, key: crate::key::Key, recursive: bool) {
    let obj = state.get_obj(key).unwrap();

    conn.put(key, &obj);

    if recursive {
        for key in obj.links() {
            put(conn, state, key, true);
        }
    }
}
