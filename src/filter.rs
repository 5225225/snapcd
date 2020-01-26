use std::fs::DirEntry;
use std::path::PathBuf;

pub fn make_filter_fn<T: AsRef<str>>(
    excludes: &[T],
    db_path: PathBuf,
) -> Box<dyn Fn(&DirEntry) -> bool> {
    let mut excl_globs = globset::GlobSetBuilder::new();

    for exclude in excludes {
        excl_globs.add(globset::Glob::new(exclude.as_ref()).unwrap());
    }

    let excl_globset = excl_globs.build().unwrap();

    Box::new(move |direntry: &DirEntry| -> bool {
        let path = direntry.path();

        let canon_path = std::fs::canonicalize(&path).unwrap();
        let canon_db_path = std::fs::canonicalize(&db_path).unwrap();

        if canon_path.starts_with(canon_db_path) {
            return false;
        }

        let normalised_path = if path.starts_with("./") {
            path.strip_prefix("./").unwrap()
        } else {
            &path
        };

        !excl_globset.is_match(normalised_path)
    })
}
