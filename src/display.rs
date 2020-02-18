use crate::{DataStore, KeyBuf};

pub struct ShownObject {

}

pub struct ShowError {

}

pub fn show_object(ds: impl DataStore, key: KeyBuf) -> Result<ShownObject, ShowError> {
    todo!();
}
