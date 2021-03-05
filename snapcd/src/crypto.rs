use std::convert::TryInto;

use aes_gcm_siv::aead::{Aead, NewAead};
use aes_gcm_siv::Aes256GcmSiv;

pub struct RepoKey([u8; 32]);

impl std::fmt::Debug for RepoKey {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_tuple("RepoKey").field(&"_").finish()
    }
}

pub struct EncryptionKey(Aes256GcmSiv);

impl std::fmt::Debug for EncryptionKey {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_tuple("EncryptionKey").field(&"_").finish()
    }
}

pub struct GearHashTable(pub [u64; 256]);

impl std::fmt::Debug for GearHashTable {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_tuple("GearHashTable").field(&"_").finish()
    }
}

impl RepoKey {
    pub fn generate() -> RepoKey {
        RepoKey(rand::random())
    }

    pub fn zero_key() -> RepoKey {
        RepoKey([0_u8; 32])
    }

    pub fn derive_encryption_key(&self) -> EncryptionKey {
        let mut output = [0_u8; 32];
        blake3::derive_key(
            "snapcd.rs 2021-03-01 21:38:07 Encryption Key",
            &self.0,
            &mut output,
        );

        EncryptionKey(Aes256GcmSiv::new(&output.into()))
    }

    pub fn derive_gearhash_table(&self) -> GearHashTable {
        let mut output = [0_u8; 256 * 8];
        blake3::derive_key(
            "snapcd.rs 2021-03-01 21:38:07 gearhash table",
            &self.0,
            &mut output,
        );

        let mut u64table = [0_u64; 256];

        for (idx, value) in output.chunks_exact(8).enumerate() {
            let ar: [u8; 8] = value.try_into().expect("incorrect number of values");
            let num = u64::from_be_bytes(ar);
            u64table[idx] = num;
        }

        GearHashTable(u64table)
    }
}

const NONCE: [u8; 12] = [0_u8; 12_usize];

impl EncryptionKey {
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        self.0
            .encrypt(&NONCE.into(), data)
            .expect("encryption failure!")
    }

    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        self.0
            .decrypt(&NONCE.into(), data)
            .expect("decryption failure!")
    }
}
