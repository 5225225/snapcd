//! The crypto module handles all the cryptography in libsnapcd
//!
//! # High level overview
//!
//! Every single repository has a mandatory encryption key that is used. This is a [`RepoKey`].
//!
//! Deduplication of objects only works between repositories using the same [`RepoKey`].
//!
//! Chunks are encrypted with AES-GCM-SIV with an [`EncryptionKey`] which is derived from this
//! [`RepoKey`].
//!
//! There is also content-defined-chunking, and in an attempt to avoid watermarking attacks, where
//! attacker defined data being inserted into the repository can leave obvious fingerprints in the
//! sizes of chunks, a [`GearHashTable`] is created. This affects where chunks are split, but this
//! is not a problem for deduplication, as every chunk is encrypted before its identifier hash is
//! taken anyways.
//!
//! # Threat Model
//!
//! For a given repository, without knowledge of the [`RepoKey`], you cannot concretely read any
//! information about what is stored inside of the repository. However, you may be able to infer
//! data based on chunk sizes.
//!
//! Additionally, with write access to a given repository, even *with* full knowledge of the
//! [`RepoKey`], it is not possible to tamper with it in such a way that there are multiple
//! possible chunks that have the same hash. This means there are no collision attacks in the hash
//! used, you can rely on a hash to have only one possible chunk it resolves to, even a hash
//! provided by a hostile user.

use std::convert::TryInto;

use aes_gcm_siv::{
    aead::{Aead, NewAead},
    Aes256GcmSiv,
};

/// The root repository key.
///
/// Never to be used directly, only subkeys are derived from this.
pub struct RepoKey([u8; 32]);

impl std::fmt::Debug for RepoKey {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_tuple("RepoKey").field(&"_").finish()
    }
}

/// The encryption key for chunks.
pub struct EncryptionKey(Aes256GcmSiv);

impl std::fmt::Debug for EncryptionKey {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_tuple("EncryptionKey").field(&"_").finish()
    }
}

/// A random table used to avoid watermark attacks.
pub struct GearHashTable(pub [u64; 256]);

impl std::fmt::Debug for GearHashTable {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_tuple("GearHashTable").field(&"_").finish()
    }
}

impl RepoKey {
    /// The all-zero key. Does not provide any protection, used for public repositories.
    pub const ZERO: Self = Self([0_u8; 32]);

    /// Generates a securely random [`RepoKey`] from system entropy.
    ///
    /// # Panics
    ///
    /// Will panic if [`getrandom::getrandom`] returns an error.
    #[must_use]
    pub fn generate() -> Self {
        let mut dest = [0_u8; 32];
        getrandom::getrandom(&mut dest).unwrap();
        Self(dest)
    }

    /// Creates an [`EncryptionKey`] from this key. Deterministic.
    #[must_use]
    pub fn derive_encryption_key(&self) -> EncryptionKey {
        let output = blake3::derive_key("snapcd.rs 2021-03-01 21:38:07 Encryption Key", &self.0);

        EncryptionKey(Aes256GcmSiv::new(&output.into()))
    }

    /// Creates a [`GearHashTable`] from this key. Deterministic.
    #[must_use]
    pub fn derive_gearhash_table(&self) -> GearHashTable {
        let mut hasher =
            blake3::Hasher::new_derive_key("snapcd.rs 2021-03-01 21:38:07 gearhash table");
        hasher.update(&self.0);
        let mut xof = hasher.finalize_xof();
        let mut output = [0_u8; 256 * 8];
        xof.fill(&mut output);

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
    /// Encrypts some data with this key.
    ///
    /// # Panics
    ///
    /// Will panic on encryption failure.
    #[must_use]
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        self.0
            .encrypt(&NONCE.into(), data)
            .expect("encryption failure!")
    }

    /// Decrypts some data with this key.
    ///
    /// # Panics
    ///
    /// Will panic on decryption failure.
    #[must_use]
    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        self.0
            .decrypt(&NONCE.into(), data)
            .expect("decryption failure!")
    }
}
