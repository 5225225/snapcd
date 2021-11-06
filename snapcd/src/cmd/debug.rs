use crate::cmd::{CmdResult, CommandTrait, DatabaseNotFoundError, State};
use libsnapcd::{commit, dir, ds::DataStore, ds::Reflog, keyish::Keyish};
use std::path::PathBuf;
use structopt::StructOpt;

#[enum_dispatch::enum_dispatch]
pub trait DebugCommandTrait {
    fn execute(self, state: &mut State) -> CmdResult;
}

#[enum_dispatch::enum_dispatch(DebugCommandTrait)]
#[derive(StructOpt, Debug)]
pub enum DebugCommand {
    PrettyPrint(PrettyPrintArgs),
    CommitTree(CommitTreeArgs),
    ReflogGet(ReflogGetArgs),
    ReflogPush(ReflogPushArgs),
    WalkTree(WalkTreeArgs),
    WalkFsTree(WalkFsTreeArgs),
    SetHead(SetHeadArgs),
    GetHead(GetHeadArgs),
}

impl CommandTrait for DebugCommand {
    fn execute(self, state: &mut State) -> CmdResult {
        DebugCommandTrait::execute(self, state)
    }
}

#[derive(StructOpt, Debug)]
pub struct SetHeadArgs {
    refname: String,
}

impl DebugCommandTrait for SetHeadArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        ds_state.ds.put_head(&self.refname)?;

        Ok(())
    }
}

impl DebugCommandTrait for GetHeadArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let head = ds_state.ds.get_head()?;
        println!("head: {:?}", head);

        Ok(())
    }
}

#[derive(StructOpt, Debug)]
pub struct WalkTreeArgs {
    key: Keyish,
}

#[derive(StructOpt, Debug)]
pub struct WalkFsTreeArgs {
    path: PathBuf,
}

#[derive(StructOpt, Debug)]
pub struct PrettyPrintArgs {
    key: Keyish,
}

#[derive(StructOpt, Debug)]
pub struct ReflogGetArgs {
    refname: String,
    remote: Option<String>,
}

#[derive(StructOpt, Debug)]
pub struct ReflogPushArgs {
    key: Keyish,
    refname: String,
    remote: Option<String>,
}

#[derive(StructOpt, Debug)]
pub struct CommitTreeArgs {
    tree: Keyish,
    parents: Vec<Keyish>,
}

#[derive(StructOpt, Debug)]
pub struct GetHeadArgs {}

impl DebugCommandTrait for WalkFsTreeArgs {
    fn execute(self, _state: &mut State) -> CmdResult {
        let fs_items = dir::walk_real_fs_items(&self.path, &|_| true)?;

        for item in fs_items {
            println!("{:?}, {}", item.0, item.1)
        }

        Ok(())
    }
}

impl DebugCommandTrait for PrettyPrintArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let key = ds_state.ds.canonicalize(self.key)?;

        let bytes = ds_state.ds.get(key)?;

        println!("cbor diagnostic:\n{}\n", minicbor::display(&bytes));

        let item = ds_state.ds.get_obj(key)?;

        item.debug_pretty_print()?;

        Ok(())
    }
}

impl DebugCommandTrait for CommitTreeArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let tree = ds_state.ds.canonicalize(self.tree)?;

        let mut parents = Vec::with_capacity(self.parents.len());

        for parent in self.parents {
            let key = ds_state.ds.canonicalize(parent)?;
            parents.push(key);
        }

        let attrs = libsnapcd::object::CommitAttrs::default();

        let commit = commit::commit_tree(&mut ds_state.ds, tree, parents, attrs)?;

        println!("{}", commit);

        Ok(())
    }
}

impl DebugCommandTrait for WalkTreeArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let key = ds_state.ds.canonicalize(self.key)?;

        let fs_items = dir::walk_fs_items(&ds_state.ds, key)?;

        for item in fs_items {
            println!("{:?}", item)
        }

        Ok(())
    }
}

impl DebugCommandTrait for ReflogGetArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let key = ds_state
            .ds
            .reflog_get(&self.refname, self.remote.as_deref())?;

        println!("{}", key);

        Ok(())
    }
}

impl DebugCommandTrait for ReflogPushArgs {
    fn execute(self, state: &mut State) -> CmdResult {
        let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

        let key = ds_state.ds.canonicalize(self.key)?;

        let log = Reflog {
            key,
            refname: self.refname,
            remote: self.remote,
        };

        ds_state.ds.reflog_push(&log)?;

        Ok(())
    }
}
