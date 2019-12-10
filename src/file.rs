use crate::{DataStore, Key, KeyBuf, Object};
use cdc::RollingHash64;
use std::borrow::Cow;
use std::io::prelude::*;

use failure::Fallible;

pub fn put_data<DS: DataStore, R: Read>(ds: &mut DS, data: R) -> Fallible<KeyBuf> {
    let mut key_bufs: [Vec<KeyBuf>; 5] = Default::default();

    let mut current_chunk = Vec::new();

    let mut hasher = cdc::Rabin64::new(6);

    for byte_r in data.bytes() {
        let byte = byte_r?;

        current_chunk.push(byte);
        hasher.slide(&byte);

        if current_chunk.len() < 1 << BLOB_ZERO_COUNT_MIN {
            continue;
        }

        let h = !hasher.get_hash();

        let zeros = h.trailing_zeros();

        if zeros > BLOB_ZERO_COUNT || current_chunk.len() >= 1 << (BLOB_ZERO_COUNT_MAX) {
            hasher.reset();

            let key = ds.put_obj(&Object::only_data(
                Cow::Borrowed(&current_chunk),
                Cow::Borrowed("file.blob"),
            ))?;
            key_bufs[0].push(key);
            current_chunk.clear();

            for offset in 0..4 {
                let len = key_bufs[offset as usize].len();
                if zeros > BLOB_ZERO_COUNT + (offset + 1) * PER_LEVEL_COUNT
                    || len >= 1 << PER_LEVEL_COUNT_MAX
                {
                    let key = ds.put_obj(&Object::only_keys(
                        Cow::Borrowed(&key_bufs[offset as usize]),
                        Cow::Borrowed("file.blobtree"),
                    ))?;
                    key_bufs[offset as usize].clear();
                    key_bufs[offset as usize + 1].push(key);
                } else {
                    break;
                }
            }
        }
    }

    if (0..4).all(|x| key_bufs[x].is_empty()) {
        // No chunks were made.
        return Ok(ds.put_obj(&Object::only_data(Cow::Borrowed(&current_chunk), Cow::Borrowed("file.blob")))?);
    }

    if !current_chunk.is_empty() {
        let key = ds.put_obj(&Object::only_data(
            Cow::Borrowed(&current_chunk),
            Cow::Borrowed("file.blob"),
        ))?;
        key_bufs[0].push(key);
    }

    for offset in 0..4 {
        let key = ds.put_obj(&Object::only_keys(
            Cow::Borrowed(&key_bufs[offset]),
            Cow::Borrowed("file.blobtree"),
        ))?;


        if key_bufs[offset].len() == 1 && (1+offset..4).all(|x| key_bufs[x].is_empty()) {
            // We know this is safe because key_bufs[offset] has exactly 1 element
            #[allow(clippy::option_unwrap_used)]
            return Ok(key_bufs[offset].pop().unwrap());
        }

        key_bufs[offset + 1].push(key);
    }

    ds.put_obj(&Object::only_keys(
        Cow::Borrowed(&key_bufs[4]),
        Cow::Borrowed("file.blobtree"),
    ))
}

pub fn read_data<DS: DataStore, W: Write>(ds: &DS, key: Key, to: &mut W) -> Fallible<()> {
    let obj = ds.get_obj(key)?;

    match &*obj.objtype {
        "file.blobtree" => {
            for key in obj.keys.iter() {
                read_data(ds, key.as_key(), to)?;
            }
        }
        "file.blob" => {
            to.write_all(&obj.data)?;
        }
        _ => {
            panic!(
                "found invalid object type {:?} when reading key {:?}",
                obj.objtype, key
            );
        }
    }

    Ok(())
}

const BLOB_ZERO_COUNT_MIN: u32 = BLOB_ZERO_COUNT - 2;
const BLOB_ZERO_COUNT: u32 = 12;
const BLOB_ZERO_COUNT_MAX: u32 = BLOB_ZERO_COUNT + 2;

const PER_LEVEL_COUNT: u32 = 5;
const PER_LEVEL_COUNT_MAX: u32 = PER_LEVEL_COUNT + 2;
