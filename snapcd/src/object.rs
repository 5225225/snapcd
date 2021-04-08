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

struct Stdout;

impl std::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result<(), std::fmt::Error> {
        print!("{}", s);

        Ok(())
    }
}

impl Object {
    pub fn debug_pretty_print(&self) -> Result<(), std::fmt::Error> {
        pretty_print(self, Stdout)
    }

    pub fn tree(&self, from: Key) -> Option<Key> {
        match self {
            Object::Commit { tree, .. } => Some(*tree),
            Object::FsItemDir { .. } => Some(from),
            Object::FsItemFile { .. } => Some(from),
            _ => None,
        }
    }

    pub fn links(&self) -> Vec<Key> {
        match self {
            Object::FileBlobTree { keys } => keys.clone(),
            Object::FileBlob { .. } => vec![],
            Object::Commit { tree, parents, .. } => {
                let mut ret = vec![*tree];
                ret.extend(parents);
                ret
            }
            Object::FsItemDir { children } => children.iter().map(|x| x.1).collect(),
            Object::FsItemFile { blob_tree, .. } => vec![*blob_tree],
        }
    }
}

fn pretty_print(obj: &Object, mut to: impl std::fmt::Write) -> Result<(), std::fmt::Error> {
    match obj {
        Object::FileBlobTree { keys } => {
            writeln!(to, "FileBlobTree:")?;
            for key in keys {
                writeln!(to, "    {}", key)?;
            }
        }
        Object::FileBlob { buf } => {
            writeln!(to, "FileBlob:")?;
            pretty_hex::pretty_hex_write(&mut to, &buf)?;
        }
        Object::Commit {
            tree,
            parents,
            attrs,
        } => {
            writeln!(to, "FileBlob:")?;
            writeln!(to, "Tree: {}", tree)?;
            writeln!(to, "Parents:")?;
            for key in parents {
                writeln!(to, "    {}", key)?;
            }
            writeln!(to, "Attrs: {:?}", attrs)?;
        }
        Object::FsItemDir { children } => {
            writeln!(to, "FsItemDir:")?;
            for (path, key) in children {
                writeln!(to, "{}: {}", path.display(), key)?;
            }
        }
        Object::FsItemFile { size, blob_tree } => {
            writeln!(to, "FsItemFile:")?;
            writeln!(to, "Size: {}", size)?;
            writeln!(to, "Key: {}", blob_tree)?;
        }
    }

    Ok(())
}
