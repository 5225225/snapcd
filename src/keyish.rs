use thiserror::Error;

#[derive(Debug)]
pub enum Keyish {
    /// Strictly speaking, this is for prefix searches
    ///
    /// .0 will be a value for which all keys that match the prefix will be lexographically ordered
    /// afterwards. For display, an encoded form of .0 should be used.
    Range(String, Vec<u8>, Option<Vec<u8>>),

    /// An exact key.
    Key(String, Vec<u8>),

    Reflog {
        orig: String,
        remote: Option<String>,
        keyname: String,
    },
}

#[derive(Debug, Error)]
pub enum KeyishParseError {
    #[error("{0} is an invalid key")]
    Invalid(String),

    #[error("{0} is an unknown format prefix")]
    UnknownPrefix(char),

    #[error("no key was given")]
    Empty,
}

impl std::str::FromStr for Keyish {
    type Err = KeyishParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains('/') {
            return parse_from_ref(s);
        } else {
            return parse_from_base32(s);
        }

        fn parse_from_ref(s: &str) -> Result<Keyish, KeyishParseError> {
            let idx = s
                .find('/')
                .expect("should only be called if s contains a /");

            if idx == 0 {
                Ok(Keyish::Reflog {
                    orig: s.to_string(),
                    keyname: s[1..].to_string(),
                    remote: None,
                })
            } else {
                let remote = &s[0..idx];
                let keyname = &s[idx + 1..];

                Ok(Keyish::Reflog {
                    orig: s.to_string(),
                    keyname: keyname.to_string(),
                    remote: Some(remote.to_string()),
                })
            }
        }

        fn parse_from_base32(s: &str) -> Result<Keyish, KeyishParseError> {
            if !s.is_ascii() {
                return Err(KeyishParseError::Invalid(s.to_string()));
            }

            // All prefixes and base32 will be in ASCII, so this is fine for indexing.
            let s_bytes: &[u8] = s.as_ref();

            let (prefix, bytes) = (s_bytes.get(0), s.get(1..).ok_or(KeyishParseError::Empty)?);

            let max_len = match prefix {
                Some(b'b') => 32 * 8,
                Some(ch) => return Err(KeyishParseError::UnknownPrefix(*ch as char)),
                _ => return Err(KeyishParseError::Invalid(s.to_string())),
            };

            let input = match crate::base32::from_base32(bytes, max_len) {
                Ok(v) => v,
                Err(_) => return Err(KeyishParseError::Invalid(s.to_string())),
            };

            if input.len() == max_len {
                let mut v = input.into_vec();
                v.insert(0_usize, 1_u8);
                return Ok(Keyish::Key(s.to_string(), v));
            }

            let did_overflow = input.all();

            let start = input.clone();

            let mut ret_start = start.into_vec();

            ret_start.insert(0_usize, 1);

            let ret_end = if did_overflow {
                None
            } else {
                let mut end = input;

                for idx in end.len()..0 {
                    if end[idx] {
                        *end.get_mut(idx).unwrap() = false;
                    } else {
                        break;
                    }
                }

                let mut v = end.into_vec();

                v.insert(0_usize, 1);

                Some(v)
            };

            Ok(Keyish::Range(s.to_string(), ret_start, ret_end))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    proptest::proptest! {
        #[test]
        fn keyish_parse_doesnt_crash(string: String) {
            let _ = Keyish::from_str(&string);
        }

        #[test]
        fn keyish_ref_parse_doesnt_crash(first: String, last: String) {
            let result = format!("{}/{}", first, last);
            let _ = Keyish::from_str(&result);
        }
    }
}
