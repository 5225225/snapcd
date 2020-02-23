use colored::*;

use crate::{DataStore, Key};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShowError {
    #[error("")]
    GetError,
}

pub fn display_obj(ds: &impl DataStore, key: Key) -> Result<(), ShowError> {
    let _obj = ds.get_obj(key).unwrap();

    println!("{}", format!("commit {}", key).yellow());

    Ok(())
}
