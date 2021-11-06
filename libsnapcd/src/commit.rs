use crate::{
    ds::DataStore,
    key::Key,
    object::{CommitAttrs, Object},
};

#[allow(clippy::implicit_hasher)]
pub fn commit_tree<DS: DataStore>(
    ds: &mut DS,
    tree: Key,
    mut parents: Vec<Key>,
    attrs: CommitAttrs,
) -> anyhow::Result<Key> {
    parents.sort();

    let commit = Object::Commit {
        tree,
        parents,
        attrs,
    };

    let ret = ds.put_obj(&commit)?;

    Ok(ret)
}
