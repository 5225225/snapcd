use crate::crypto::GearHashTable;
use crate::ds;
use crate::{ds::DataStore, key::Key, object::Object};
use std::io::prelude::*;
use thiserror::Error;

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
    mut data: R,
    table: &GearHashTable,
    mut put_data: impl FnMut(&[u8]) -> Result<Key, ds::PutObjError>,
    mut put_keys: impl FnMut(&[Key]) -> Result<Key, ds::PutObjError>,
) -> anyhow::Result<Key> {
    let mut key_bufs: [Vec<Key>; 5] = Default::default();

    let mut read_buffer = [0u8; 1 << 16usize];
    let mut chunk_buffer: Vec<u8> = Vec::new();
    let mut current_chunk = Vec::new();

    let mut hasher = gearhash::Hasher::new(&table.0);

    loop {
        let m = {
            let hasher_match = hasher.next_match(&chunk_buffer, BLOB_ZERO_COUNT_BITMASK);

            if current_chunk.len() > 1 << BLOB_ZERO_COUNT_MAX {
                // We've gone on too long, force a cut here.
                Some(((1 << BLOB_ZERO_COUNT_MAX) as usize).saturating_sub(current_chunk.len()))
            } else {
                hasher_match.map(|x| x.min((1 << BLOB_ZERO_COUNT_MAX) - current_chunk.len()))
            }
        };

        if let Some(boundry) = m {
            current_chunk.extend_from_slice(&chunk_buffer[0..boundry]);
            chunk_buffer.drain(0..boundry);

            let zeros = hasher.get_hash().leading_zeros();

            debug_assert!(zeros >= BLOB_ZERO_COUNT || boundry == (1 << BLOB_ZERO_COUNT_MAX));

            debug_assert!(current_chunk.len() <= 1<<BLOB_ZERO_COUNT_MAX, "tried to put a too long chunk in, it was {} bytes, we need it to be less than or equal to {}", current_chunk.len(), 1<<BLOB_ZERO_COUNT_MAX);
            let key = put_data(&current_chunk)?;

            key_bufs[0].push(key);
            current_chunk.clear();

            for offset in 0..4 {
                let len = key_bufs[offset as usize].len();
                if zeros > BLOB_ZERO_COUNT + (offset + 1) * PER_LEVEL_COUNT
                    || len >= 1 << PER_LEVEL_COUNT_MAX
                {
                    let key = put_keys(&key_bufs[offset as usize])?;
                    key_bufs[offset as usize].clear();
                    key_bufs[offset as usize + 1].push(key);
                } else {
                    break;
                }
            }
        } else {
            use std::io::ErrorKind;

            let boundry =
                ((1 << BLOB_ZERO_COUNT_MAX as usize) - current_chunk.len()).min(chunk_buffer.len());

            current_chunk.extend_from_slice(&chunk_buffer[0..boundry]);
            chunk_buffer.drain(0..boundry);

            match data.read(&mut read_buffer) {
                Ok(len) => {
                    if len == 0 {
                        break;
                    }
                    chunk_buffer.extend_from_slice(&read_buffer[0..len]);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }

    current_chunk.extend_from_slice(&chunk_buffer);

    if (0..4).all(|x| key_bufs[x].is_empty()) {
        // No chunks were made.
        return Ok(put_data(&current_chunk)?);
    }

    if !current_chunk.is_empty() {
        let key = put_data(&current_chunk)?;
        key_bufs[0].push(key);
    }

    for offset in 0..4 {
        let key = put_keys(&key_bufs[offset])?;

        if key_bufs[offset].len() == 1 && (1 + offset..4).all(|x| key_bufs[x].is_empty()) {
            // We know this is safe because key_bufs[offset] has exactly 1 element
            #[allow(clippy::unwrap_used)]
            return Ok(key_bufs[offset].pop().unwrap());
        }

        key_bufs[offset + 1].push(key);
    }

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

const BLOB_ZERO_COUNT: u32 = 13;
const BLOB_ZERO_COUNT_MAX: u32 = BLOB_ZERO_COUNT + 2;

const BLOB_ZERO_COUNT_BITMASK: u64 = !((1 << (64 - BLOB_ZERO_COUNT)) - 1);

const PER_LEVEL_COUNT: u32 = 7;
const PER_LEVEL_COUNT_MAX: u32 = PER_LEVEL_COUNT + 2;
