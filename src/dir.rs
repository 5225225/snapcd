use std::borrow::Cow;
use std::path::{Path};
use std::ffi::{OsString};
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
        let value = serde_cbor::value::to_value(&self).unwrap();

        dbg!(&value);

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
        let item: serde_cbor::Value = serde_cbor::from_slice(&self.data).unwrap();

        dbg!(&item);

        let mut fsitem: FSItem = serde_cbor::value::from_value(item).unwrap();

        fsitem.children = self.keys.into_owned();

        Ok(fsitem)
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
            itemtype: FSItemType::File,
            size: meta.len(),
        };

        let object = obj.try_into().unwrap();

        return ds.put_obj(&object);
    }

    unimplemented!("meta is not a file or a directory?")
}

pub fn get_fs_item<DS: DataStore>(ds: &DS, key: Key, path: &Path) {
    let obj = ds.get_obj(key);

    let fsobj: FSItem = obj.try_into().unwrap();

    match fsobj.itemtype {
        FSItemType::Dir => {
            for child in fsobj.children {
                get_fs_item(ds, child.as_key(), &path.join(&fsobj.name))
            }
        }
        FSItemType::File => {
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(dbg!(path.join(&fsobj.name))).unwrap();

            file::read_data(ds, fsobj.children[0].as_key(), &mut f);
        }
    }
}
