use std::fs::DirEntry;
use std::path::PathBuf;

pub fn make_filter_fn<T: AsRef<str>>(
    excludes: &[T],
    db_path: &Option<PathBuf>,
) -> Box<dyn Fn(&DirEntry) -> bool> {
    let mut excl_globs = globset::GlobSetBuilder::new();

    for exclude in excludes {
        excl_globs.add(globset::Glob::new(exclude.as_ref()).unwrap());
    }

    let excl_globset = excl_globs.build().unwrap();

    let cloned_db_path: Option<PathBuf> = db_path.clone();

    Box::new(move |direntry: &DirEntry| -> bool {
        let path = direntry.path();

        if let Some(p) = &cloned_db_path {
            let canon_path = std::fs::canonicalize(&path).unwrap();
            let canon_p = std::fs::canonicalize(&p).unwrap();

            if canon_path.starts_with(canon_p) {
                return false;
            }
        }

        let normalised_path = if path.starts_with("./") {
            path.strip_prefix("./").unwrap()
        } else {
            &path
        };

        !excl_globset.is_match(normalised_path)
    })
}
