/// Represents an existing thing on the file system, either a directory or a file.
#[derive(Debug)]
pub enum Entry {
    Dir(cap_std::fs::Dir),
    File(cap_std::fs::File),
}

impl From<cap_std::fs::Dir> for Entry {
    fn from(dir: cap_std::fs::Dir) -> Self {
        Self::Dir(dir)
    }
}

impl From<cap_std::fs::File> for Entry {
    fn from(file: cap_std::fs::File) -> Self {
        Self::File(file)
    }
}

impl Entry {
    /// Creates an [`Entry`] from a path. Will not follow symlinks, and will panic if used on one.
    pub fn from_path(path: &std::path::Path, authority: cap_std::AmbientAuthority) -> Self {
        let item = std::fs::metadata(path).unwrap();

        if item.is_dir() {
            let cap_dir = cap_std::fs::Dir::open_ambient_dir(&path, authority).unwrap();
            Entry::Dir(cap_dir)
        } else if item.is_file() {
            let file = std::fs::File::open(&path).unwrap();
            let cap_file = cap_std::fs::File::from_std(file, authority);
            Entry::File(cap_file)
        } else {
            panic!(
                "Unrecognised item type for path {}, metadata is {:?}",
                path.display(),
                item
            );
        }
    }

    pub fn from_direntry(entry: &cap_std::fs::DirEntry) -> Self {
        let file_type = entry.file_type().unwrap();

        if file_type.is_dir() {
            Entry::Dir(entry.open_dir().unwrap())
        } else if file_type.is_file() {
            Entry::File(entry.open().unwrap())
        } else {
            panic!("Unrecognised item type for entry {:?}", entry);
        }
    }
}
