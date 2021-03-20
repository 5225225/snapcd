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
}

#[derive(StructOpt, Debug)]
pub struct PutCommand {
    key: crate::keyish::Keyish,
}

impl NetCommandTrait for GetCommand {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

        let conn = crate::network::Connection {
            url: "http://localhost:8000".to_string(),
        };

        let data = conn.get(self.key);

        ds_state.ds.put_obj(&data).unwrap();

        Ok(())
    }
}

impl NetCommandTrait for PutCommand {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_ref().ok_or(DatabaseNotFoundError)?;

        let conn = crate::network::Connection {
            url: "http://localhost:8000".to_string(),
        };

        let k = ds_state.ds.canonicalize(self.key).unwrap();

        let obj = ds_state.ds.get_obj(k).unwrap();

        conn.put(k, obj);

        Ok(())
    }
}
