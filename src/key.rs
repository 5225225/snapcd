#[derive(
    Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum KeyBuf {
    Blake3B([u8; 32]),
}

impl KeyBuf {
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

    pub fn from_db_key(x: &[u8]) -> Self {
        use std::convert::TryInto;

        let hash_id = x[0];
        let hash_bytes = &x[1..];

        match hash_id {
            1 => Self::Blake3B(hash_bytes.try_into().unwrap()),
            0 | 2..=255 => panic!("invalid key"),
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

impl std::fmt::Display for KeyBuf {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        fmt.write_str(&self.as_user_key())
    }
}
