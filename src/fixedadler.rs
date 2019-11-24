use std::io;
use adler32::*;

pub struct FixedSizeAdler32 {
    adler32: adler32::RollingAdler32,
    buffer: Box<[u8]>,
    position: usize,
    size: usize,
}

impl FixedSizeAdler32 {
    /// Creates an empty Adler32 context with a buffer of the given size.
    pub fn new(size: usize) -> FixedSizeAdler32 {
        Self::from_value(size, 1)
    }

    /// Creates an empty Adler32 context with the given size and value.
    pub fn from_value(size: usize, adler32: u32) -> FixedSizeAdler32 {
        FixedSizeAdler32 {
            adler32: RollingAdler32::from_value(adler32),
            buffer: vec![0; size].into_boxed_slice(),
            position: 0,
            size: 0,
        }
    }

    pub fn hash(&self) -> u32 {
        self.adler32.hash()
    }
}

impl io::Write for FixedSizeAdler32 {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Keep total size, which will be returned
        let total_size = buf.len();
        let buffer_size = self.buffer.len();

        // Truncate the given array to the size of our internal buffer
        let size = if total_size > buffer_size {
            buffer_size
        } else {
            total_size
        };
        let buf = &buf[(total_size - size)..];

        // Remove old bytes
        let start = buffer_size + self.position - self.size;
        let remove = if self.size + size > buffer_size {
            self.size + size - buffer_size
        } else {
            0
        };
        for i in 0..remove {
            self.adler32.remove(
                self.size - i,
                self.buffer[(start + i) % buffer_size]);
        }
        self.size -= remove;

        // Add new bytes
        self.adler32.update_buffer(buf);
        self.size += size;
        if self.position + size > buffer_size {
            self.buffer[self.position..]
                .clone_from_slice(
                    &buf[..(buffer_size - self.position)]);
            self.buffer[..(size + self.position - buffer_size)]
                .clone_from_slice(
                    &buf[(buffer_size - self.position)..]);
        } else {
            self.buffer[self.position..(self.position + size)]
                .clone_from_slice(&buf);
        }
        self.position = (self.position + size) % buffer_size;
        Ok(total_size)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.write(buf).map(|_| ())
    }
}
