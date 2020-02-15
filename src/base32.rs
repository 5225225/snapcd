
use bitvec::prelude::*;
use blake3::hash;
use failure_derive::Fail;
use std::path::Path;

use rusqlite::params;
use rusqlite::OptionalExtension;
use std::borrow::Cow;

use failure::Fallible;

fn pop_u5_from_bitvec(x: &mut BitVec<Msb0, u8>) -> u8 {
    let mut v = 0;

    v |= (*x.get(0).unwrap_or(&false) as u8) << 4;
    v |= (*x.get(1).unwrap_or(&false) as u8) << 3;
    v |= (*x.get(2).unwrap_or(&false) as u8) << 2;
    v |= (*x.get(3).unwrap_or(&false) as u8) << 1;
    v |= (*x.get(4).unwrap_or(&false) as u8) << 0;

    for _ in 0..5 {
        if !x.is_empty() {
            x.remove(0_usize);
        }
    }

    assert!(v <= 31);

    v
}

#[derive(Debug, Fail)]
pub enum FromBase32Error {
    #[fail(display = "found non-base32 char {}", _0)]
    UnknownByte(char),
}

pub fn from_base32(x: &str, max_len: usize) -> Fallible<BitVec<Msb0, u8>> {
    let mut result = BitVec::<Msb0, u8>::new();

    for mut ch in x.bytes() {
        if (b'A'..=b'Z').contains(&ch) {
            ch |= 0b0010_0000; // Convert to lowercase
        }

        let idx = TABLE
            .iter()
            .position(|&x| x == ch)
            .ok_or_else(|| FromBase32Error::UnknownByte(ch as char))?;

        debug_assert!((ch as char).is_ascii_lowercase() || (ch as char).is_ascii_digit());

        result.push(idx & 0b10000 != 0);
        result.push(idx & 0b01000 != 0);
        result.push(idx & 0b00100 != 0);
        result.push(idx & 0b00010 != 0);
        result.push(idx & 0b00001 != 0);
    }

    result.truncate(max_len);

    Ok(result)
}

static TABLE: [u8; 32] = *b"abcdefghijklmnopqrstuvwxyz234567";

pub fn to_base32(x: &[u8]) -> String {
    let mut scratch = BitVec::<Msb0, u8>::from_vec(x.to_vec());
    let mut ret = String::new();
    while !scratch.is_empty() {
        let v = pop_u5_from_bitvec(&mut scratch);
        ret.push(TABLE[v as usize] as char);
    }

    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest::proptest! {
#[test]
        fn round_trip_base32(bytes: Vec<u8>) {
            let b32 = to_base32(&bytes);
            let restored = from_base32(&b32, bytes.len() * 8).unwrap();
            assert_eq!(restored.as_slice(), &*bytes);
        }
    }
}
