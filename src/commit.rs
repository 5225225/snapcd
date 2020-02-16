use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;
use thiserror::Error;

use failure::Fallible;

use crate::{DataStore, KeyBuf, Object};

struct Commit {
    tree: KeyBuf,
    parents: Vec<KeyBuf>,
    attrs: HashMap<String, String>,
}

impl TryInto<Object<'static>> for Commit {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Object<'static>, serde_cbor::error::Error> {
        let attrs = serde_cbor::to_vec(&self.attrs)?;

        let mut keys = vec![];
        keys.push(self.tree);
        keys.extend(self.parents);

        Ok(Object {
            data: Cow::Owned(serde_bytes::ByteBuf::from(attrs)),
            keys: Cow::Owned(keys),
            objtype: Cow::Borrowed("commit.commit"),
        })
    }
}

impl TryInto<Commit> for Object<'static> {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Commit, serde_cbor::error::Error> {
        let item: serde_cbor::Value = serde_cbor::from_slice(&self.data)?;
        let attrs: HashMap<String, String> = serde_cbor::value::from_value(item)?;

        let mut owned_keys = self.keys.into_owned();

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
    Serialisation {
        #[from] source: serde_cbor::error::Error,
    },
}

#[allow(clippy::implicit_hasher)]
pub fn commit_tree<DS: DataStore>(
    ds: &mut DS,
    tree: KeyBuf,
    mut parents: Vec<KeyBuf>,
    attrs: HashMap<String, String>,
) -> Fallible<KeyBuf> {
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
