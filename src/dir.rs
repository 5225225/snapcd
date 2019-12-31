use crate::{file, DataStore, KeyBuf, Object};
use std::borrow::Cow;
use std::convert::TryInto;
use std::ffi::OsString;
use std::fs::DirEntry;
use std::path::Path;

use failure::Fallible;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FSItem {
    name: OsString,
    size: u64,
    itemtype: FSItemType,
    #[serde(skip)]
    children: Vec<KeyBuf>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum FSItemType {
    Dir,
    File,
}

impl TryInto<Object<'static>> for FSItem {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<Object<'static>, serde_cbor::error::Error> {
        let value = serde_cbor::value::to_value(&self)?;

        let obj = serde_cbor::to_vec(&value)?;

        let objtype = match self.itemtype {
            FSItemType::Dir => "dir.FSItem.dir",
            FSItemType::File => "dir.FSItem.file",
        };

        Ok(Object {
            data: Cow::Owned(obj),
            keys: Cow::Owned(self.children),
            objtype: Cow::Borrowed(objtype),
        })
    }
}

impl<'a> TryInto<FSItem> for Object<'a> {
    type Error = serde_cbor::error::Error;

    fn try_into(self) -> Result<FSItem, serde_cbor::error::Error> {
        let item: serde_cbor::Value = serde_cbor::from_slice(&self.data)?;

        let mut fsitem: FSItem = serde_cbor::value::from_value(item)?;

        fsitem.children = self.keys.into_owned();

        Ok(fsitem)
    }
}

pub fn put_fs_item<DS: DataStore>(
    ds: &mut DS,
    path: &Path,
    filter: &dyn Fn(&DirEntry) -> bool,
) -> Fallible<KeyBuf> {
    internal_put_fs_item(ds, path, filter, true)
}

pub fn internal_put_fs_item<DS: DataStore>(
    ds: &mut DS,
    path: &Path,
    filter: &dyn Fn(&DirEntry) -> bool,
    is_root: bool,
) -> Fallible<KeyBuf> {
    let meta = std::fs::metadata(path)?;

    if meta.is_dir() {
        let mut result = Vec::new();

        let entries = std::fs::read_dir(path)?;

        for entry in entries {
            match entry {
                Ok(direntry) => {
                    if filter(&direntry) {
                        result.push(internal_put_fs_item(ds, &direntry.path(), filter, false)?);
                    }
                }
                Err(e) => Err(e)?,
            }
        }

        let size = result.len() as u64;

        let name;

        if is_root {
            name = OsString::new();
        } else {
            if let Some(f) = path.file_name() {
                name = f.to_os_string();
            } else {
                let canon = path.canonicalize()?;

                name = canon
                    .file_name()
                    .expect("we tried to canonicalize the filename and it still has no name")
                    .to_os_string();
            }
        }

        let obj = FSItem {
            name,
            children: result,
            itemtype: FSItemType::Dir,
            size,
        };

        let object = obj.try_into()?;

        return Ok(ds.put_obj(&object)?);
    }

    if meta.is_file() {
        let f = std::fs::File::open(path)?;

        let reader = std::io::BufReader::new(f);

        let hash = file::put_data(ds, reader)?;

        #[allow(clippy::option_unwrap_used)]
        // From the docs: "Returns None if the path terminates in `..`."
        // Therefore, this can never fail since we will only execute this if it's a valid filename.
        let fname = path.file_name().unwrap();

        let obj = FSItem {
            name: fname.to_os_string(),
            children: vec![hash],
            itemtype: FSItemType::File,
            size: meta.len(),
        };

        let object = obj.try_into()?;

        return Ok(ds.put_obj(&object)?);
    }

    unimplemented!("meta is not a file or a directory?")
}

pub fn get_fs_item<DS: DataStore>(ds: &DS, key: &KeyBuf, path: &Path) -> Fallible<()> {
    let obj = ds.get_obj(key)?;

    let fsobj: FSItem = obj.try_into()?;

    match fsobj.itemtype {
        FSItemType::Dir => {
            for child in fsobj.children {
                get_fs_item(ds, &child, &path.join(&fsobj.name))?;
            }
        }
        FSItemType::File => {
            let fpath = &path.join(&fsobj.name);

            if let Some(parent) = fpath.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(fpath)?;

            file::read_data(ds, &fsobj.children[0], &mut f)?;
        }
    }

    Ok(())
}
