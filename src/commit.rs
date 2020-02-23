use std::collections::HashMap;
use std::convert::TryInto;
use thiserror::Error;

use crate::{DataStore, Key, Object};

use crate::ds::PutObjError;
use crate::object::ObjType;

struct Commit {
    tree: Key,
    parents: Vec<Key>,
    attrs: HashMap<String, String>,
}

impl TryInto<Object<'static>> for Commit {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Object<'static>, serde_cbor::error::Error> {
        let attrs = serde_cbor::to_vec(&self.attrs)?;

        let mut keys = vec![];
        keys.push(self.tree);
        keys.extend(self.parents);

        Ok(Object::new_owned(attrs, keys, ObjType::Commit))
    }
}

impl TryInto<Commit> for Object<'static> {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Commit, serde_cbor::error::Error> {
        let item: serde_cbor::Value = serde_cbor::from_slice(&self.data())?;
        let attrs: HashMap<String, String> = serde_cbor::value::from_value(item)?;

        let mut owned_keys = self.keys().to_vec();

        let tree = owned_keys.remove(0);
        let parents = owned_keys;

        Ok(Commit {
            tree,
            parents,
            attrs,
        })
    }
}

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
    attrs: HashMap<String, String>,
) -> Result<Key, CommitTreeError> {
    parents.sort();

    let commit = Commit {
        tree,
        parents,
        attrs,
    };

    let val: Object = commit.try_into()?;

    let ret = ds.put_obj(&val)?;

    Ok(ret)
}
