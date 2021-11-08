use std::convert::TryInto;

use thiserror::Error;

#[derive(
    Debug, Clone, Copy, minicbor::Encode, minicbor::Decode, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Key {
    #[n(0)]
    Blake3B(
        #[n(0)]
        #[cbor(with = "minicbor::bytes")]
        [u8; 32],
    ),
}

#[derive(Debug, Error)]
pub enum FromDbKeyError {
    #[error("Unknown hash id {_0}")]
    UnknownHashId(u8),

    #[error("No bytes were given")]
    Empty,

    #[error("Incorrect length, got {got} hash bytes")]
    IncorrectLength {
        got: usize,
        #[source]
        source: std::array::TryFromSliceError,
    },
}

#[derive(Debug, Error)]
pub enum FromUserKeyError {
    #[error("Unknown hash id {_0}")]
    UnknownHashPrefix(char),

    #[error("No bytes were given")]
    Empty,

    #[error("Invalid base32")]
    FromBase32(#[from] crate::base32::DecodeError),

    #[error("Incorrect length, got {got} hash bytes")]
    IncorrectLength {
        got: usize,
        #[source]
        source: std::array::TryFromSliceError,
    },
}

impl std::str::FromStr for Key {
    type Err = FromUserKeyError;

    fn from_str(s: &str) -> Result<Self, FromUserKeyError> {
        // All prefixes and base32 will be in ASCII, so this is fine for indexing.
        let s_bytes: &[u8] = s.as_ref();

        let (prefix, bytes) = (
            *s_bytes.get(0).ok_or(FromUserKeyError::Empty)?,
            s.get(1..).ok_or(FromUserKeyError::Empty)?,
        );

        match prefix as char {
            'b' => {
                let hash_bytes = crate::base32::decode(bytes, 32 * 8)?.into_vec();

                let hash_arr = match (&hash_bytes[..]).try_into() {
                    Ok(a) => a,
                    Err(e) => {
                        return Err(FromUserKeyError::IncorrectLength {
                            got: hash_bytes.len(),
                            source: e,
                        })
                    }
                };

                Ok(Self::Blake3B(hash_arr))
            }
            c => Err(FromUserKeyError::UnknownHashPrefix(c)),
        }
    }
}

impl Key {
    fn hash_id(&self) -> u8 {
        match self {
            Self::Blake3B(_) => 1,
        }
    }

    fn hash_bytes(&self) -> &[u8] {
        match self {
            Self::Blake3B(x) => x.as_ref(),
        }
    }

    pub fn from_db_key(x: &[u8]) -> Result<Self, FromDbKeyError> {
        if x.is_empty() {
            return Err(FromDbKeyError::Empty);
        }

        let hash_id = x[0];
        let hash_bytes = &x[1..];

        match hash_id {
            1 => {
                let hash_arr = match hash_bytes.try_into() {
                    Ok(a) => a,
                    Err(e) => {
                        return Err(FromDbKeyError::IncorrectLength {
                            got: hash_bytes.len(),
                            source: e,
                        })
                    }
                };
                Ok(Self::Blake3B(hash_arr))
            }
            0 | 2..=255 => Err(FromDbKeyError::UnknownHashId(hash_id)),
        }
    }

    #[must_use]
    pub fn as_db_key(&self) -> Vec<u8> {
        let hash_id = self.hash_id();
        let hash_bytes = self.hash_bytes();

        let mut result = Vec::with_capacity(hash_bytes.len() + 1);

        result.push(hash_id);
        result.extend(hash_bytes);

        result
    }

    #[must_use]
    pub fn as_user_key(&self) -> String {
        let mut result = String::new();

        let prefix = match self {
            Self::Blake3B(_) => "b",
        };

        result.push_str(prefix);

        let encoded = crate::base32::encode(self.hash_bytes());

        result.push_str(&encoded);

        result
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(&self.as_user_key())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{key::Key, keyish::Keyish};

    proptest::proptest! {
        #[test]
        fn from_db_key_doesnt_panic(bytes: Vec<u8>) {
            let _ = Key::from_db_key(&bytes);
        }

        #[test]
        fn parse_doesnt_panic(s: String) {
            let _ = Key::from_str(&s);
        }

        #[test]
        fn round_trip_blake3b_user(bytes: [u8; 32]) {
            let k = Key::Blake3B(bytes);
            let as_db = k.as_user_key();
            let from_db = Key::from_str(&as_db);
            assert_eq!(k, from_db.expect("failed to parse db key"));
        }

        #[test]
        fn from_db_key_round_trip(bytes: Vec<u8>) {
            if let Ok(key) = Key::from_db_key(&bytes) {
                let db_key = key.as_db_key();
                assert_eq!(db_key, bytes);
            }
        }

        #[test]
        fn round_trip_blake3b_db(bytes: [u8; 32]) {
            let k = Key::Blake3B(bytes);
            let as_db = k.as_db_key();
            let from_db = Key::from_db_key(&as_db);
            assert_eq!(k, from_db.expect("failed to parse db key"));
        }

        #[test]
        fn round_trip_blake3b_to_keyish(bytes: [u8; 32]) {
            let k = Key::Blake3B(bytes);
            let as_user = k.as_user_key();
            let from_user = Keyish::from_str(&as_user).unwrap();


            if let Keyish::Key(_, b) = from_user {
                assert_eq!(b, k.as_db_key(), "keyish from_str did not round trip with as_user_key");

                let db_key = Key::from_db_key(&b);
                if let Ok(Key::Blake3B(newbytes)) = db_key {
                    assert_eq!(bytes, newbytes);
                } else {
                    panic!("parsed a keybuf that wasn't expected hash type");
                }
            } else {
                panic!("we asked for the full key and we got something else");
            }
        }
    }
}
