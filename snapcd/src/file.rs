use crate::crypto::GearHashTable;
use crate::ds;
use crate::{ds::DataStore, key::Key, object::Object};

use std::io::prelude::*;

use itertools::Itertools;
use thiserror::Error;
use tracing::trace;

#[derive(Debug, Error)]
pub enum PutDataError {
    #[error("error putting object: {_0}")]
    PutObjError(#[from] ds::PutObjError),

    #[error("io error: {_0}")]
    IoError(#[from] std::io::Error),
}

pub fn put_data<DS: DataStore, R: Read>(ds: &mut DS, data: R) -> anyhow::Result<Key> {
    let table = ds.get_gearhash_table();

    inner_put_data(
        data,
        table,
        &mut |data: &[u8]| ds.put_obj(&Object::FileBlob { buf: data.to_vec() }),
        &mut |keys: &[Key]| {
            ds.put_obj(&Object::FileBlobTree {
                keys: keys.to_vec(),
            })
        },
    )
}

pub fn inner_put_data<R: Read>(
    data: R,
    table: &GearHashTable,
    mut put_data: impl FnMut(&[u8]) -> Result<Key, ds::PutObjError>,
    mut put_keys: impl FnMut(&[Key]) -> Result<Key, ds::PutObjError>,
) -> anyhow::Result<Key> {
    let mut chunker = libsnapcd::chunker::Chunker::with_table(data, &table.0);

    let mut key_bufs: [Vec<Key>; 5] = Default::default();

    while let Some(chunk) = chunker.next_chunk()? {
        trace!("putting chunk of length {}", chunk.buf().len());
        let key = put_data(chunk.buf())?;

        key_bufs[0].push(key);

        for offset in 0..4 {
            let len = key_bufs[offset as usize].len();

            if chunk.depth() > (offset + 1) * PER_LEVEL_COUNT || len >= 1 << PER_LEVEL_COUNT_MAX {
                trace!("putting keys depth {} of length {}", offset, len);
                let key = put_keys(&key_bufs[offset as usize])?;
                key_bufs[offset as usize].clear();
                key_bufs[offset as usize + 1].push(key);
            } else {
                break;
            }
        }
    }

    if (0..4).all(|x| key_bufs[x].is_empty()) {
        // We didn't find any chunks at all, so input was empty.
        return Ok(put_data(b"")?);
    }

    for offset in 0..4 {
        // If there's no items "ahead" of us...
        if (1 + offset..4).all(|x| key_bufs[x].is_empty()) {
            // and we only have one item in *this* tree...
            if let [k] = *key_bufs[offset] {
                debug_assert!(key_bufs.iter().flatten().exactly_one().ok() == Some(&k));

                // We can just return it.
                return Ok(k);
            }
        }

        trace!(
            "putting keys depth {} of length {}",
            offset,
            &key_bufs[offset].len()
        );
        let key = put_keys(&key_bufs[offset])?;
        key_bufs[offset].clear();

        key_bufs[offset + 1].push(key);
    }

    trace!("putting keys depth 4 of length {}", &key_bufs[4].len());
    Ok(put_keys(&key_bufs[4])?)
}

#[derive(Debug, Error)]
pub enum ReadDataError {
    #[error("error putting object: {_0}")]
    GetObjError(#[from] ds::GetObjError),

    #[error("io error: {_0}")]
    IoError(#[from] std::io::Error),
}

pub fn read_data<DS: DataStore, W: Write>(
    ds: &DS,
    key: Key,
    to: &mut W,
) -> Result<(), ReadDataError> {
    let obj = ds.get_obj(key)?;

    match obj {
        Object::FileBlobTree { keys } => {
            for key in keys {
                read_data(ds, key, to)?;
            }
        }
        Object::FileBlob { buf } => {
            to.write_all(&buf)?;
        }
        Object::FsItemFile { blob_tree, .. } => {
            read_data(ds, blob_tree, to)?;
        }
        _ => {
            panic!("found invalid object {:?} when reading key {:?}", obj, key);
        }
    }

    Ok(())
}

const PER_LEVEL_COUNT: u32 = 6;
const PER_LEVEL_COUNT_MAX: u32 = 9;
