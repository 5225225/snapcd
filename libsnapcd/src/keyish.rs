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
pub enum ParseError {
    #[error("{0} is an invalid key")]
    Invalid(String),

    #[error("{0} is an unknown format prefix")]
    UnknownPrefix(char),

    #[error("no key was given")]
    Empty,
}

impl std::str::FromStr for Keyish {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn parse_from_ref(s: &str) -> Keyish {
            let idx = s
                .find('/')
                .expect("should only be called if s contains a /");

            if idx == 0 {
                Keyish::Reflog {
                    orig: s.to_owned(),
                    keyname: s[1..].to_owned(),
                    remote: None,
                }
            } else {
                let remote = &s[0..idx];
                let keyname = &s[idx + 1..];

                Keyish::Reflog {
                    orig: s.to_owned(),
                    keyname: keyname.to_owned(),
                    remote: Some(remote.to_owned()),
                }
            }
        }

        fn parse_from_base32(s: &str) -> Result<Keyish, ParseError> {
            if !s.is_ascii() {
                return Err(ParseError::Invalid(s.to_owned()));
            }

            // All prefixes and base32 will be in ASCII, so this is fine for indexing.
            let s_bytes: &[u8] = s.as_ref();

            let (prefix, bytes) = (s_bytes.get(0), s.get(1..).ok_or(ParseError::Empty)?);

            let max_len = match prefix {
                Some(b'b') => 32 * 8,
                Some(ch) => return Err(ParseError::UnknownPrefix(*ch as char)),
                _ => return Err(ParseError::Invalid(s.to_owned())),
            };

            let input = match crate::base32::decode(bytes, max_len) {
                Ok(v) => v,
                Err(_) => return Err(ParseError::Invalid(s.to_owned())),
            };

            if input.len() == max_len {
                let mut v = input.into_vec();
                v.insert(0_usize, 1_u8);
                return Ok(Keyish::Key(s.to_owned(), v));
            }

            let did_overflow = input.all();

            let start = input.clone();

            let mut ret_start = start.into_vec();

            ret_start.insert(0_usize, 1);

            let ret_end = if did_overflow {
                None
            } else {
                let mut end = input;

                for idx in (0..end.len()).rev() {
                    if end[idx] {
                        *end.get_mut(idx).unwrap() = false;
                    } else {
                        *end.get_mut(idx).unwrap() = true;
                        break;
                    }
                }

                let mut v = end.into_vec();

                v.insert(0_usize, 1);

                Some(v)
            };

            Ok(Keyish::Range(s.to_owned(), ret_start, ret_end))
        }

        if s.contains('/') {
            Ok(parse_from_ref(s))
        } else {
            parse_from_base32(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    proptest::proptest! {
        #[test]
        fn keyish_parse_doesnt_crash(string: String) {
            drop(Keyish::from_str(&string));
        }

        #[test]
        fn keyish_ref_parse_doesnt_crash(first: String, last: String) {
            let result = format!("{}/{}", first, last);
            drop(Keyish::from_str(&result));
        }
    }
}
