use thiserror::Error;

use crate::object::CommitAttrs;
use crate::{DataStore, Object};
use libsnapcd::key::Key;

use crate::ds::PutObjError;

#[derive(Debug, Error)]
pub enum CommitTreeError {
    #[error("error when serialising object")]
    SerialisationError(#[from] serde_cbor::error::Error),

    #[error("error when putting object")]
    PutObjError(#[from] PutObjError),
}

#[allow(clippy::implicit_hasher)]
pub fn commit_tree<DS: DataStore>(
    ds: &mut DS,
    tree: Key,
    mut parents: Vec<Key>,
    attrs: CommitAttrs,
) -> Result<Key, CommitTreeError> {
    parents.sort();

    let commit = Object::Commit {
        tree,
        parents,
        attrs,
    };

    let ret = ds.put_obj(&commit)?;

    Ok(ret)
}
