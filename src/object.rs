use serde_bytes;
use std::borrow::Cow;
use crate::KeyBuf;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Object<'a> {
    pub data: Cow<'a, serde_bytes::Bytes>,
    pub keys: Cow<'a, [KeyBuf]>,
    pub objtype: Cow<'a, str>,
}

#[derive(Copy, Clone)]
pub enum ObjectShowFormat {
    Oneline,
    Message,
    Stat,
    Full
}

impl<'a> Object<'a> {
    pub fn new(data: &'a [u8], keys: &'a [KeyBuf], objtype: &'static str) -> Self {
        Self {
            data: Cow::Borrowed(serde_bytes::Bytes::new(data)),
            keys: Cow::Borrowed(keys),
            objtype: Cow::Borrowed(objtype),
        }
    }

    pub fn debug_pretty_print(&self) -> impl std::fmt::Display + '_ {
        ObjectPrettyPrinter(self)
    }

    pub fn show(&'a self, ds: &'a dyn crate::DataStore) -> impl std::fmt::Display + 'a {
        ObjectShowPrinter(self, ds)
    }
}

struct ObjectPrettyPrinter<'a>(&'a Object<'a>);
const DISPLAY_CHUNK_SIZE: usize = 20;
impl<'a> std::fmt::Display for ObjectPrettyPrinter<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        writeln!(fmt, "--type: {:?}--", self.0.objtype)?;

        writeln!(fmt, "--keys--")?;
        if !self.0.keys.is_empty() {
            for key in self.0.keys.iter() {
                writeln!(fmt, "{}", key)?;
            }
        }
        writeln!(fmt, "-/keys--")?;

        writeln!(fmt, "--data--")?;
        if !self.0.data.is_empty() {
            for chunk in self.0.data.chunks(DISPLAY_CHUNK_SIZE) {
                let ashex = hex::encode(chunk);
                writeln!(fmt, "{}", ashex)?;
            }
        }
        writeln!(fmt, "-/data--")?;

        writeln!(fmt, "--deserialised data--")?;

        match serde_cbor::from_slice::<serde_cbor::Value>(&self.0.data) {
            Ok(v) => {
                println!("{:?}", v);
            }
            Err(e) => {
                println!("error when deserialising!");
                println!("{:?}", e);
            }
        };
        writeln!(fmt, "--/deserialised data--")?;

        Ok(())
    }
}

struct ObjectShowPrinter<'a>(&'a Object<'a>, &'a dyn crate::DataStore);
impl<'a> std::fmt::Display for ObjectShowPrinter<'a> {
    fn fmt(&self, _fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self.0.objtype {
            _ => {
                debug_assert!(false, "unable to format {}", self.0.objtype);
                Err(std::fmt::Error)
            }
        }
    }
}

