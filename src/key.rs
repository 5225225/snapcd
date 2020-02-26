use thiserror::Error;

#[derive(
    Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Key {
    Blake3B([u8; 32]),
}

#[derive(
    serde::Serialize, serde::Deserialize
)]
pub struct TypedKey<T> {
    inner: Key,

    #[serde(skip)]
    _marker: std::marker::PhantomData<T>,
}

impl<T> std::fmt::Debug for TypedKey<T> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> { 
        self.inner.fmt(fmt)
    }
}
impl<T> std::clone::Clone for TypedKey<T> {
    fn clone(&self) -> Self {*self}
}
impl<T> std::cmp::PartialEq for TypedKey<T> {
    fn eq(&self, other: &Self) -> bool {self.inner.eq(&other.inner)}
}
impl<T> std::cmp::Eq for TypedKey<T> {}

impl<T> std::cmp::PartialOrd for TypedKey<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {self.inner.partial_cmp(&other.inner)}
}
impl<T> std::cmp::Ord for TypedKey<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {self.inner.cmp(&other.inner)}
}

impl<T> std::hash::Hash for TypedKey<T> {
 fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher {
        self.inner.hash(state)
        }
}
impl<T> std::marker::Copy for TypedKey<T> {}

impl<T> TypedKey<T> {
    pub fn inner(&self) -> Key {
        self.inner
    }
}

impl<T> std::convert::Into<TypedKey<T>> for Key {
    fn into(self) -> TypedKey<T> {
        TypedKey {
            inner: self,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> std::convert::Into<Key> for TypedKey<T> {
    fn into(self) -> Key {
        self.inner
    }
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
        use std::convert::TryInto;

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

    pub fn as_db_key(&self) -> Vec<u8> {
        let hash_id = self.hash_id();
        let hash_bytes = self.hash_bytes();

        let mut result = Vec::with_capacity(hash_bytes.len() + 1);

        result.push(hash_id);
        result.extend(hash_bytes);

        result
    }

    pub fn as_user_key(&self) -> String {
        let mut result = String::new();

        let prefix = match self {
            Self::Blake3B(_) => "b",
        };

        result.push_str(prefix);

        let encoded = crate::base32::to_base32(self.hash_bytes());

        result.push_str(&encoded);

        result
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        fmt.write_str(&self.as_user_key())
    }
}

impl<T> std::fmt::Display for TypedKey<T> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        fmt.write_str(&self.inner.as_user_key())
    }
}

#[cfg(test)]
mod tests {
    use crate::{key::Key, Keyish};
    use std::str::FromStr;

    proptest::proptest! {
        #[test]
        fn from_db_key_doesnt_panic(bytes: Vec<u8>) {
            let _ = Key::from_db_key(&bytes);
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
                    panic!("parsed a keybuf that wasn't expected hash type")
                }
            } else {
                panic!("we asked for the full key and we got something else")
            }
        }
    }
}
