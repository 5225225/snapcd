use crate::key::Key;
use std::borrow::ToOwned;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Object {
    data: serde_bytes::ByteBuf,
    keys: Vec<Key>,
    objtype: ObjType,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ObjType {
    FileBlobTree,
    FileBlob,
    Commit,
    FSItemDir,
    FSItemFile,

    #[serde(other)]
    Unknown,
}

#[derive(Debug, Copy, Clone)]
pub enum ObjectShowFormat {
    Oneline,
    Message,
    Stat,
    Full,
}

impl Object {
    pub fn into_owned(self) -> Object {
        self
    }

    pub fn new(data: &[u8], keys: &[Key], objtype: ObjType) -> Self {
        Self {
            data: serde_bytes::ByteBuf::from(data),
            keys: keys.to_owned(),
            objtype,
        }
    }

    pub fn new_owned(data: Vec<u8>, keys: Vec<Key>, objtype: ObjType) -> Self {
        Self {
            data: serde_bytes::ByteBuf::from(data),
            keys,
            objtype,
        }
    }

    pub fn debug_pretty_print(&self) -> Result<(), std::io::Error> {
        pretty_print(self, std::io::stdout().lock())
    }

    pub fn objtype(&self) -> ObjType {
        self.objtype
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

const DISPLAY_CHUNK_SIZE: usize = 20;

// In tests, you should force the color output to be true or not
// This will assume `to` is stdout and will color based on that (and envars)
// see: https://docs.rs/colored/1.9.2/colored/control/index.html
fn pretty_print(obj: &Object, mut to: impl std::io::Write) -> Result<(), std::io::Error> {
    writeln!(to, "--type: {:?}--", obj.objtype)?;

    writeln!(to, "--keys--")?;
    if !obj.keys.is_empty() {
        for key in obj.keys.iter() {
            writeln!(to, "{}", key)?;
            writeln!(to, "{:?}", key)?;
            writeln!(to)?;
        }
    }
    writeln!(to, "-/keys--")?;

    writeln!(to, "--data--")?;
    if !obj.data.is_empty() {
        for chunk in obj.data.chunks(DISPLAY_CHUNK_SIZE) {
            let ashex = hex::encode(chunk);
            writeln!(to, "{}", ashex)?;
        }
    }
    writeln!(to, "-/data--")?;

    writeln!(to, "--deserialised data--")?;

    match serde_cbor::from_slice::<serde_cbor::Value>(&obj.data) {
        Ok(v) => {
            writeln!(to, "{:?}", v)?;
        }
        Err(e) => {
            writeln!(to, "error when deserialising!")?;
            writeln!(to, "{:?}", e)?;
        }
    };
    writeln!(to, "--/deserialised data--")?;

    Ok(())
}
