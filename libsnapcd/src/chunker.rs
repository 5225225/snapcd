//! A fast content-defined chunker over a reader.
//!
//! More detailed docs are available on [`Chunker`]

use std::io::ErrorKind;
use std::io::Read;

use static_assertions::const_assert_eq;

/// A chunker is an implementation of the FastCDC algorithm described in
/// <https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf>.
#[derive(Debug)]
pub struct Chunker<'t, R> {
    reader: R,
    buf: Vec<u8>,
    min_size: usize,
    normal_size: usize,
    max_size: usize,
    hasher: gearhash::Hasher<'t>,
    need_to_erase: usize,
}

// If we need to read anything, read this much.
const READ_SIZE: usize = 8 * 1024;

const MASK_S: u64 = 0xfffe_0000_0000_0000;
const MASK_L: u64 = 0xffe0_0000_0000_0000;

const_assert_eq!(MASK_S.count_ones(), 15); // 15 '1' bits.
const_assert_eq!(MASK_S.leading_ones(), 15); // all at the start

const_assert_eq!(MASK_L.count_ones(), 11); // 11 '1' bits.
const_assert_eq!(MASK_L.leading_ones(), 11); // all at the start

impl<'t, R> Chunker<'t, R> {
    /// Creates a chunker using a specified gearhash table.
    pub fn with_table(reader: R, table: &'t gearhash::Table) -> Self {
        Chunker {
            reader,
            hasher: gearhash::Hasher::new(table),
            buf: Vec::new(),
            min_size: 256,
            normal_size: 8192,
            max_size: 65535,
            need_to_erase: 0,
        }
    }
}

impl<'t, R: Read> Chunker<'t, R> {
    /// Reads the next chunk if it exists, otherwise returns None.
    ///
    /// # Errors
    ///
    /// If the reader returns an error when read that is not [`ErrorKind::Interrupted`], that error
    /// is returned, and no changes will have been made to the chunker.
    pub fn next_chunk(&mut self) -> Result<Option<Chunk<'_>>, std::io::Error> {
        if self.need_to_erase != 0 {
            self.buf.drain(0..self.need_to_erase);
            self.need_to_erase = 0;
        }

        self.try_fill_buffer()?;

        if self.buf.is_empty() {
            return Ok(None);
        }

        self.hasher.set_hash(0);

        let buf = &self.buf[0..(self.max_size.min(self.buf.len()))];

        if buf.len() <= self.min_size {
            self.hasher.update(buf);
            self.need_to_erase = buf.len();

            return Ok(Some(Chunk {
                buf,
                hash: self.hasher.get_hash(),
            }));
        }

        let normal_buf_len = self.normal_size.min(buf.len());
        let normal_buf = &buf[0..normal_buf_len];

        if let Some(cut) = self.hasher.next_match(normal_buf, MASK_S) {
            self.need_to_erase = cut;
            return Ok(Some(Chunk {
                buf: &buf[0..cut],
                hash: self.hasher.get_hash(),
            }));
        }

        // We've now updated the hasher with 0..normal_size and haven't yet found a match.
        let rest = &buf[normal_buf_len..];

        if let Some(cut) = self.hasher.next_match(rest, MASK_L) {
            self.need_to_erase = normal_buf_len + cut;
            return Ok(Some(Chunk {
                buf: &buf[0..normal_buf_len + cut],
                hash: self.hasher.get_hash(),
            }));
        }

        self.need_to_erase = buf.len();

        Ok(Some(Chunk {
            buf,
            hash: self.hasher.get_hash(),
        }))
    }

    /// Tries to fill the in-memory buffer.
    ///
    /// Once this returns, you can be guaranteed that either
    ///
    /// 1. `buf.len()` is more than or equal to `buf.max_size`
    /// 2. We are at EOF.
    fn try_fill_buffer(&mut self) -> Result<(), std::io::Error> {
        let mut buf = [0_u8; READ_SIZE];

        while buf.len() < self.max_size {
            match self.reader.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(l) => {
                    self.buf.extend_from_slice(&buf[0..l]);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

/// A chunk is returned from [`Chunker::next_chunk()`].
#[derive(Debug)]
pub struct Chunk<'a> {
    buf: &'a [u8],
    hash: u64,
}

impl<'a> Chunk<'a> {
    /// Returns the buffer contained in this chunk.
    ///
    /// Will always be non-empty.
    #[must_use]
    pub fn buf(&self) -> &'a [u8] {
        debug_assert!(!self.buf.is_empty());

        self.buf
    }

    /// Returns a number specifying the depth in the hash that split this chunk.
    ///
    /// This can be used to implement trees. Having a specific depth value gets half as likely (a
    /// depth of 4 is 1/16, a depth of 5 is 1/32).
    #[must_use]
    pub fn depth(&self) -> u32 {
        self.hash.trailing_ones()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::convert::TryFrom;
    use std::io::Read;

    #[test]
    fn reproduce() {
        #[rustfmt::skip]
        let reader = blake3::Hasher::new_keyed(&[
            0xf0, 0x9f, 0x8f, 0xb3, 0xef, 0xb8, 0x8f, 0xe2,
            0x80, 0x8d, 0xe2, 0x9a, 0xa7, 0xef, 0xb8, 0x8f,
            0x54, 0x72, 0x61, 0x6e, 0x73, 0x20, 0x52, 0x69,
            0x67, 0x68, 0x74, 0x73, 0x21, 0x20, 0x3c, 0x33,
        ]).finalize_xof().take(1024*1024*8);

        let mut chunker = Chunker::with_table(reader, &gearhash::DEFAULT_TABLE);

        let mut seen = blake3::Hasher::new();

        let mut total_len = 0;

        while let Some(chunk) = chunker.next_chunk().unwrap() {
            total_len += chunk.buf().len();
            seen.update(&u32::try_from(chunk.buf().len()).unwrap().to_le_bytes());
            seen.update(&chunk.depth().to_le_bytes());
        }

        let final_result = seen.finalize();

        assert_eq!(
            final_result.to_string(),
            "596fc6f00a845bd277d2f6856425ee00a8fc3077429b80bcaacba8df1d093ae6"
        );

        assert_eq!(total_len, 1024 * 1024 * 8);
    }
}
