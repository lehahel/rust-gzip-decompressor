#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::io::{self, Write};

use anyhow::{anyhow, bail, Result};
use crc::{Crc, Digest, CRC_32_ISO_HDLC};

////////////////////////////////////////////////////////////////////////////////

const HISTORY_SIZE: usize = 32768;

#[warn(dead_code)]
pub const CRC_CFG: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

pub struct TrackingWriter<T> {
    inner: T,
    buf: VecDeque<u8>,
    bytes_counter: usize,
    crc_digest: Digest<'static, u32>,
}

impl<T: Write> Write for TrackingWriter<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.write(buf) {
            Ok(size) => {
                for item in buf.iter().take(size) {
                    if self.buf.len() >= HISTORY_SIZE {
                        self.buf.pop_front();
                    }
                    self.buf.push_back(*item);
                }
                self.crc_digest.update(&buf[0..size]);
                self.bytes_counter += size;
                Ok(size)
            }
            err => err,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf.clear();
        self.inner.flush()
    }
}

impl<T: Write> TrackingWriter<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            buf: VecDeque::<u8>::new(),
            bytes_counter: 0usize,
            crc_digest: CRC_CFG.digest(),
        }
    }

    /// Write a sequence of `len` bytes written `dist` bytes ago.
    pub fn write_previous(&mut self, dist: usize, len: usize) -> Result<()> {
        // println!(
        //     "writing previous (dist = {}, len = {}, buffer_size = {})",
        //     dist,
        //     len,
        //     self.buf.len()
        // );
        if dist > self.buf.len() {
            bail!("bad dist");
            // bail!("length check failed crc32 check failed unsupported compression method");
        }
        let mut buf = vec![];
        for idx in 0..len {
            buf.push(self.buf[self.buf.len() - dist + (idx % dist)]);
        }
        match self.write(buf.as_slice()) {
            Ok(size) if size < len => Err(anyhow!("buffer overflow")),
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow::Error::new(err)),
        }
    }

    pub fn byte_count(&self) -> usize {
        self.bytes_counter
    }

    pub fn crc32(self) -> u32 {
        self.crc_digest.finalize()
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    #[test]
    fn write() -> Result<()> {
        let mut buf: &mut [u8] = &mut [0u8; 10];
        let mut writer = TrackingWriter::new(&mut buf);

        assert_eq!(writer.write(&[1, 2, 3, 4])?, 4);
        assert_eq!(writer.byte_count(), 4);

        assert_eq!(writer.write(&[4, 8, 15, 16, 23])?, 5);
        assert_eq!(writer.byte_count(), 9);

        assert_eq!(writer.write(&[0, 0, 123])?, 1);
        assert_eq!(writer.byte_count(), 10);

        assert_eq!(writer.write(&[42, 124, 234, 27])?, 0);
        assert_eq!(writer.byte_count(), 10);
        assert_eq!(writer.crc32(), 2992191065);

        Ok(())
    }

    #[test]
    fn write_previous() -> Result<()> {
        let mut buf: &mut [u8] = &mut [0u8; 512];
        let mut writer = TrackingWriter::new(&mut buf);

        for i in 0..=255 {
            writer.write_u8(i)?;
        }

        writer.write_previous(192, 128)?;
        assert_eq!(writer.byte_count(), 384);

        assert!(writer.write_previous(10000, 20).is_err());
        assert_eq!(writer.byte_count(), 384);

        assert!(writer.write_previous(256, 256).is_err());
        assert_eq!(writer.byte_count(), 512);

        assert!(writer.write_previous(1, 1).is_err());
        assert_eq!(writer.byte_count(), 512);
        assert_eq!(writer.crc32(), 2733545866);

        Ok(())
    }
}
