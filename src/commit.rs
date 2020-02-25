use std::collections::HashMap;
use std::convert::TryInto;
use thiserror::Error;
use crate::key::TypedKey;

use serde::{Deserialize, Serialize};

use crate::{DataStore, Key, Object};

use crate::ds::PutObjError;
use crate::object::ObjType;

use crate::dir;

pub struct Commit {
    tree: TypedKey<dir::FSItem>,
    parents: Vec<TypedKey<Commit>>,
    attrs: CommitAttrs,
}

#[derive(Serialize, Deserialize, Default)]
pub struct CommitAttrs {
    message: String,
    extra: HashMap<String, serde_cbor::Value>,
}

impl CommitAttrs {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn set_message(&mut self, msg: String) {
        self.message = msg;
    }
}

impl Commit {
    pub fn from_key(ds: &impl DataStore, key: TypedKey<Commit>) -> Self {
        ds.get_obj(key.inner()).unwrap().into_owned().try_into().unwrap()
    }

    pub fn parents(&self) -> &[TypedKey<Commit>] {
        &self.parents
    }

    pub fn tree(&self) -> TypedKey<dir::FSItem> {
        self.tree
    }

    pub fn attrs(&self) -> &CommitAttrs {
        &self.attrs
    }

    pub fn attrs_mut(&mut self) -> &mut CommitAttrs {
        &mut self.attrs
    }
}

impl TryInto<Object<'static>> for Commit {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Object<'static>, serde_cbor::error::Error> {
        let attrs = serde_cbor::to_vec(&self.attrs)?;

        let mut keys: Vec<Key> = vec![];

        keys.push(self.tree.into());

        for p in self.parents {
            keys.push(p.into())
        }

        Ok(Object::new_owned(attrs, keys, ObjType::Commit))
    }
}

impl TryInto<Commit> for Object<'static> {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Commit, serde_cbor::error::Error> {
        let item: serde_cbor::Value = serde_cbor::from_slice(&self.data())?;

        let attrs: CommitAttrs = serde_cbor::value::from_value(item)?;

        let mut owned_keys = self.keys().to_vec();

        let tree = owned_keys.remove(0);
        let parents = owned_keys;

        let mut new_parents = Vec::new();
        for parent in parents {
            new_parents.push(parent.into())
        }

        Ok(Commit {
            tree: tree.into(),
            parents: new_parents,
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
    tree: TypedKey<dir::FSItem>,
    mut parents: Vec<TypedKey<Commit>>,
    attrs: CommitAttrs,
) -> Result<TypedKey<Commit>, CommitTreeError> {
    parents.sort();

    let commit = Commit {
        tree,
        parents,
        attrs,
    };

    let val: Object = commit.try_into()?;

    let ret = ds.put_obj(&val)?;

    Ok(ret.into())
}
