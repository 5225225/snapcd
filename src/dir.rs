use std::borrow::Cow;
use std::path::{PathBuf, Path};
use std::ffi::{OsStr, OsString};
use crate::{DataStore, Key, KeyBuf, Object, file};
use std::convert::TryInto;

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
        let obj = serde_cbor::to_vec(&self)?;

        let objtype = match self.itemtype {
            FSItemType::Dir => "dir.FSItem.dir",
            FSItemType::File => "dir.FSItem.dir",
        };

        Ok(Object {
            data: Cow::Owned(obj),
            keys: Cow::Owned(self.children),
            objtype: Cow::Borrowed(objtype),
        })
    }
}

pub fn put_fs_item<DS: DataStore>(ds: &mut DS, path: &Path) -> KeyBuf {
    let meta = std::fs::metadata(path).unwrap();

    if meta.is_dir() {
        let mut result = Vec::new();

        let entries = std::fs::read_dir(path).unwrap();

        for r_entry in entries {
            let entry = r_entry.unwrap();

            result.push(put_fs_item(ds, &entry.path()));
        }

        let size = result.len() as u64;

        let obj = FSItem {
            name: path.file_name().unwrap().to_os_string(),
            children: result,
            itemtype: FSItemType::Dir,
            size,
        };

        let object = obj.try_into().unwrap();

        return ds.put_obj(&object);
    }

    if meta.is_file() {
        let f = std::fs::File::open(path).unwrap();

        let reader = std::io::BufReader::new(f);

        let hash = file::put_data(ds, reader);

        let obj = FSItem {
            name: path.file_name().unwrap().to_os_string(),
            children: vec![hash],
            itemtype: FSItemType::Dir,
            size: meta.len(),
        };

        let object = obj.try_into().unwrap();

        return ds.put_obj(&object);
    }

    unimplemented!("meta is not a file or a directory?")
}
