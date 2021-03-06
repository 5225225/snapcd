use crate::ds;
use crate::object::ObjType;
use crate::{ds::DataStore, key::Key, object::Object};
use std::io::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PutDataError {
    #[error("error putting object: {_0}")]
    PutObjError(#[from] ds::PutObjError),

    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),
}

pub fn put_data<DS: DataStore, R: Read>(ds: &mut DS, mut data: R) -> Result<Key, PutDataError> {
    let mut key_bufs: [Vec<Key>; 5] = Default::default();

    let mut read_buffer = [0u8; 1 << 16usize];
    let mut chunk_buffer: Vec<u8> = Vec::new();
    let mut current_chunk = Vec::new();

    let mut hasher = gearhash::Hasher::new(&gearhash::DEFAULT_TABLE);

    loop {
        let m = {
            let hasher_match = hasher.next_match(&chunk_buffer, BLOB_ZERO_COUNT_BITMASK);

            if chunk_buffer.len() > 1 << BLOB_ZERO_COUNT_MAX {
                // We've gone on too long, force a cut here.
                Some(1 << BLOB_ZERO_COUNT_MAX)
            } else {
                hasher_match
            }
        };

        if let Some(boundry) = m {
            current_chunk.extend_from_slice(&chunk_buffer[0..boundry]);
            chunk_buffer.drain(0..boundry);

            let zeros = hasher.get_hash().leading_zeros();

            debug_assert!(zeros >= BLOB_ZERO_COUNT || boundry == (1 << BLOB_ZERO_COUNT_MAX));

            if current_chunk.len() >= 1 << (BLOB_ZERO_COUNT_MAX) {
                let key = ds.put_obj(&Object::new(&current_chunk, &[], ObjType::FileBlob))?;
                key_bufs[0].push(key);
                current_chunk.clear();

                for offset in 0..4 {
                    let len = key_bufs[offset as usize].len();
                    if zeros > BLOB_ZERO_COUNT + (offset + 1) * PER_LEVEL_COUNT
                        || len >= 1 << PER_LEVEL_COUNT_MAX
                    {
                        let key = ds.put_obj(&Object::new(
                            &[],
                            &key_bufs[offset as usize],
                            ObjType::FileBlobTree,
                        ))?;
                        key_bufs[offset as usize].clear();
                        key_bufs[offset as usize + 1].push(key);
                    } else {
                        break;
                    }
                }
            }
        } else {
            use std::io::ErrorKind;
            current_chunk.extend_from_slice(&chunk_buffer);
            chunk_buffer.clear();

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
        return Ok(ds.put_obj(&Object::new(&current_chunk, &[], ObjType::FileBlob))?);
    }

    if !current_chunk.is_empty() {
        let key = ds.put_obj(&Object::new(&current_chunk, &[], ObjType::FileBlob))?;
        key_bufs[0].push(key);
    }

    for offset in 0..4 {
        let key = ds.put_obj(&Object::new(&[], &key_bufs[offset], ObjType::FileBlobTree))?;

        if key_bufs[offset].len() == 1 && (1 + offset..4).all(|x| key_bufs[x].is_empty()) {
            // We know this is safe because key_bufs[offset] has exactly 1 element
            #[allow(clippy::option_unwrap_used)]
            return Ok(key_bufs[offset].pop().unwrap());
        }

        key_bufs[offset + 1].push(key);
    }

    Ok(ds.put_obj(&Object::new(&[], &key_bufs[4], ObjType::FileBlobTree))?)
}

#[derive(Debug, Error)]
pub enum ReadDataError {
    #[error("error putting object: {_0}")]
    GetObjError(#[from] ds::GetObjError),

    #[error("io error: {_0}")]
    IOError(#[from] std::io::Error),
}

pub fn read_data<DS: DataStore, W: Write>(
    ds: &DS,
    key: Key,
    to: &mut W,
) -> Result<(), ReadDataError> {
    let obj = ds.get_obj(key)?;

    match obj.objtype() {
        ObjType::FileBlobTree => {
            for key in obj.keys().iter().copied() {
                read_data(ds, key, to)?;
            }
        }
        ObjType::FileBlob => {
            to.write_all(&obj.data())?;
        }
        ObjType::FSItemFile => {
            assert!(obj.keys().len() == 1);

            let key = obj.keys()[0];
            read_data(ds, key, to)?;
        }
        _ => {
            panic!(
                "found invalid object type {:?} when reading key {:?}",
                obj.objtype(),
                key
            );
        }
    }

    Ok(())
}

const BLOB_ZERO_COUNT: u32 = 13;
const BLOB_ZERO_COUNT_MAX: u32 = BLOB_ZERO_COUNT + 2;

const BLOB_ZERO_COUNT_BITMASK: u64 = !((1 << (64 - BLOB_ZERO_COUNT)) - 1);

const PER_LEVEL_COUNT: u32 = 7;
const PER_LEVEL_COUNT_MAX: u32 = PER_LEVEL_COUNT + 2;
