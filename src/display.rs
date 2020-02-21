use colored::*;

use crate::{DataStore, KeyBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShowError {
    #[error("")]
    GetError,
}


pub fn display_obj(ds: &impl DataStore, key: KeyBuf) -> Result<(), ShowError> {
    let obj = ds.get_obj(&key).unwrap();

    println!("{}", format!("commit {}", key).yellow());


    Ok(())
}
