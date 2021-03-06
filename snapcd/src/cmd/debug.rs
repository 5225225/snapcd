use std::path::PathBuf;
use structopt::{StructOpt, clap::AppSettings};
use crate::cmd::common::{State, CmdResult, DatabaseNotFoundError};
use crate::{
    cache::SqliteCache, commit, dir, ds::sqlite::SqliteDs, ds::GetReflogError, ds::Transactional,
    filter, object::Object, DataStore, Keyish, Reflog,
};

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

pub fn debug(state: &mut State, args: DebugCommand) -> CmdResult {
    match args {
        DebugCommand::PrettyPrint(args) => debug_pretty_print(state, args),
        DebugCommand::CommitTree(args) => debug_commit_tree(state, args),
        DebugCommand::ReflogGet(args) => debug_reflog_get(state, args),
        DebugCommand::ReflogPush(args) => debug_reflog_push(state, args),
        DebugCommand::WalkTree(args) => debug_walk_tree(state, args),
        DebugCommand::WalkFsTree(args) => debug_walk_fs_tree(state, args),
        DebugCommand::SetHead(args) => debug_set_head(state, args),
        DebugCommand::GetHead(args) => debug_get_head(state, args),
    }
}


#[derive(StructOpt, Debug)]
pub struct SetHeadArgs {
    refname: String,
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

fn debug_set_head(state: &mut State, args: SetHeadArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    ds_state.ds.put_head(&args.refname)?;

    Ok(())
}

fn debug_get_head(state: &mut State, _args: GetHeadArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let head = ds_state.ds.get_head()?;
    println!("head: {:?}", head);

    Ok(())
}

fn debug_walk_tree(state: &mut State, args: WalkTreeArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let fs_items = dir::walk_fs_items(&ds_state.ds, key)?;

    for item in fs_items {
        println!("{:?}", item)
    }

    Ok(())
}

fn debug_walk_fs_tree(_state: &mut State, args: WalkFsTreeArgs) -> CmdResult {
    let fs_items = dir::walk_real_fs_items(&args.path, &|_| true)?;

    for item in fs_items {
        println!("{:?}, {}", item.0, item.1)
    }

    Ok(())
}

fn debug_pretty_print(state: &mut State, args: PrettyPrintArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let item = ds_state.ds.get_obj(key)?;

    item.debug_pretty_print()?;

    Ok(())
}

fn debug_commit_tree(state: &mut State, args: CommitTreeArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let tree = ds_state.ds.canonicalize(args.tree)?;

    let mut parents = Vec::with_capacity(args.parents.len());

    for parent in args.parents {
        let key = ds_state.ds.canonicalize(parent)?;
        parents.push(key);
    }

    let attrs = crate::object::CommitAttrs::default();

    let commit = commit::commit_tree(&mut ds_state.ds, tree, parents, attrs)?;

    println!("{}", commit);

    Ok(())
}

fn debug_reflog_get(state: &mut State, args: ReflogGetArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state
        .ds
        .reflog_get(&args.refname, args.remote.as_deref())?;

    println!("{}", key);

    Ok(())
}

fn debug_reflog_push(state: &mut State, args: ReflogPushArgs) -> CmdResult {
    let ds_state = state.ds_state.as_mut().ok_or(DatabaseNotFoundError)?;

    let key = ds_state.ds.canonicalize(args.key)?;

    let log = Reflog {
        key,
        refname: args.refname,
        remote: args.remote,
    };

    ds_state.ds.reflog_push(&log)?;

    Ok(())
}
