use crate::key::Key;
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
    FsItemDir {
        children: Vec<(PathBuf, Key)>,
    },
    FsItemFile {
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

    pub fn tree(&self, from: Key) -> Option<Key> {
        match self {
            Object::Commit { tree, .. } => Some(*tree),
            Object::FsItemDir { .. } => Some(from),
            Object::FsItemFile { .. } => Some(from),
            _ => None,
        }
    }
}

fn pretty_print(obj: &Object, mut to: impl std::io::Write) -> Result<(), std::io::Error> {
    writeln!(to, "{:#?}", obj)?;

    Ok(())
}
