use crate::key::Key;
use std::path::PathBuf;

#[derive(Debug, minicbor::Encode, minicbor::Decode)]
pub enum Object {
    #[n(0)]
    FileBlobTree {
        #[n(0)]
        keys: Vec<Key>, // this is either a FileBlob or a FileBlobTree
    },
    #[n(1)]
    FileBlob {
        #[n(0)]
        #[cbor(with = "minicbor::bytes")]
        buf: Vec<u8>,
    },
    #[n(2)]
    Commit {
        #[n(0)]
        tree: Key, // FSItemDir
        #[n(1)]
        parents: Vec<Key>, // Commit
        #[n(2)]
        attrs: CommitAttrs,
    },
    #[n(3)]
    FsItemDir {
        #[n(0)]
        // TODO: Refactor to DirEntry struct
        children: Vec<(PathBuf, Key, bool)>,
    },
    #[n(4)]
    FsItemFile {
        #[n(0)]
        size: u64,
        #[n(1)]
        blob_tree: Key, // this is either FileBlobTree or a FileBlob
    },
}

#[derive(Debug, minicbor::Encode, minicbor::Decode, Default)]
pub struct CommitAttrs {
    #[n(0)]
    pub message: Option<String>,
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

    #[must_use]
    pub fn tree(&self, from: Key) -> Option<Key> {
        match self {
            Object::Commit { tree, .. } => Some(*tree),
            Object::FsItemDir { .. } | Object::FsItemFile { .. } => Some(from),
            _ => None,
        }
    }

    #[must_use]
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
            for (path, key, is_dir) in children {
                let dir_string = if *is_dir { "/ (dir)" } else { "" };
                writeln!(to, "{}: {}{}", path.display(), key, dir_string)?;
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
