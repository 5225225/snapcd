use crate::key::Key;
use std::borrow::ToOwned;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Object {
    FileBlobTree {
        keys: Vec<Key>, // this is either a FileBlob or a FileBlobTree
    },
    FileBlob {
        #[serde(with = "serde_bytes")]
        buf: Vec<u8>,
    },
    Commit {
        tree: Key,         // FSItemDir
        parents: Vec<Key>, // Commit
        attrs: CommitAttrs,
    },
    FSItemDir {
        children: Vec<(PathBuf, Key)>,
    },
    FSItemFile {
        size: u64,
        blob_tree: Key, // this is either FileBlobTree or a FileBlob
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct CommitAttrs {
    pub message: String,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_cbor::Value>,
}

impl Object {
    pub fn debug_pretty_print(&self) -> Result<(), std::io::Error> {
        pretty_print(self, std::io::stdout().lock())
    }
}

// In tests, you should force the color output to be true or not
// This will assume `to` is stdout and will color based on that (and envars)
// see: https://docs.rs/colored/1.9.2/colored/control/index.html
fn pretty_print(obj: &Object, mut to: impl std::io::Write) -> Result<(), std::io::Error> {
    writeln!(to, "{:#?}", obj);

    Ok(())
}
