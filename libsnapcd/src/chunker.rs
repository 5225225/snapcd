use std::io::ErrorKind;
use std::io::Read;

use static_assertions::const_assert_eq;

/// A chunker is an implementation of the FastCDC algorithm described in
/// <https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf>.
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

const MASK_S: u64 = 0xfffe000000000000;
const MASK_L: u64 = 0xffe0000000000000;

const_assert_eq!(MASK_S.count_ones(), 15); // 15 '1' bits.
const_assert_eq!(MASK_S.leading_ones(), 15); // all at the start

const_assert_eq!(MASK_L.count_ones(), 11); // 11 '1' bits.
const_assert_eq!(MASK_L.leading_ones(), 11); // all at the start

impl<'t, R> Chunker<'t, R> {
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

impl<R> Chunker<'static, R> {
    pub fn new(reader: R) -> Self {
        Chunker::with_table(reader, &gearhash::DEFAULT_TABLE)
    }
}

impl<'t, R: Read> Chunker<'t, R> {
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

/// A chunk is returned from [`Chunker::next`].
pub struct Chunk<'a> {
    buf: &'a [u8],
    hash: u64,
}

impl<'a> Chunk<'a> {
    /// Returns the buffer contained in this chunk.
    ///
    /// Will always be non-empty.
    pub fn buf(&self) -> &'a [u8] {
        debug_assert!(!self.buf.is_empty());

        self.buf
    }

    pub fn depth(&self) -> u32 {
        self.hash.trailing_ones()
    }
}
