//! A bit-aware base32 encoder/decoder
//!
//! Uses Bitvec in [`decode`] in order to allow for decoding into a series of bits. This
//! allows you to do easier range searches, used for [`Keyish::Range`].
//!
//! [`Keyish::Range`]: crate::keyish::Keyish::Range

use bitvec::prelude::*;
use thiserror::Error;

fn pop_u5_from_bitvec(x: &mut BitVec<Msb0, u8>) -> u8 {
    let mut v = 0;

    v |= u8::from(*x.get(0).as_deref().unwrap_or(&false)) << 4;
    v |= u8::from(*x.get(1).as_deref().unwrap_or(&false)) << 3;
    v |= u8::from(*x.get(2).as_deref().unwrap_or(&false)) << 2;
    v |= u8::from(*x.get(3).as_deref().unwrap_or(&false)) << 1;
    v |= u8::from(*x.get(4).as_deref().unwrap_or(&false));

    for _ in 0..5 {
        if !x.is_empty() {
            x.remove(0_usize);
        }
    }

    debug_assert!(v <= 31);

    v
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("found non-base32 char {0}")]
    UnknownByte(char),
}

/// Decodes the bits from `x` as a base32 string that was previously used with [`encode`] into a
/// `BitVec`.
///
/// `max_len` is used for when there were bits left over, and you do not want to decode them as
/// zero bits.
pub fn decode(x: &str, max_len: usize) -> Result<BitVec<Msb0, u8>, DecodeError> {
    let mut result = BitVec::<Msb0, u8>::new();

    for mut ch in x.bytes() {
        if (b'A'..=b'Z').contains(&ch) {
            ch |= 0b0010_0000; // Convert to lowercase
        }

        let idx = TABLE
            .iter()
            .position(|&x| x == ch)
            .ok_or(DecodeError::UnknownByte(ch as char))?;

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

/// Converts `x` into a [`String`] as base32, using the table `"abcdefghijklmnopqrstuvwxyz234567"`.
///
/// Each character represents 5 bits, and if there are any bits left over, they will be used in the
/// high bits to select an index from the table.
///
///
/// ```rust
/// use libsnapcd::base32::encode;
///
/// let table: [u8; 32] = *b"abcdefghijklmnopqrstuvwxyz234567";
///
/// let x = [0b10101_010__; 1];
///
/// let s = encode(&x);
/// let s_bytes = s.as_bytes();
///
/// let first_char = table[0b10101];
/// assert_eq!(s_bytes[0], first_char);
///
/// let second_char = table[0b01000];
/// assert_eq!(s_bytes[1], second_char);
///
/// assert_eq!(s, "vi");
/// ```
#[must_use]
pub fn encode(x: &[u8]) -> String {
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
            let b32 = encode(&bytes);
            let restored = decode(&b32, bytes.len() * 8).unwrap();
            assert_eq!(restored.as_raw_slice(), &*bytes);
        }

        #[test]
        fn from_base32_non_panicking(bytes: String, mul: usize) {
            drop(decode(&bytes, bytes.len() * (mul % 10)));
        }
    }
}
