#![forbid(unsafe_code)]

use std::io::{self, BufRead};

use byteorder::ReadBytesExt;

////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BitSequence {
    bits: u16,
    len: u8,
}

impl BitSequence {
    pub fn new(bits: u16, len: u8) -> Self {
        Self {
            bits: bits & ((1u16 << len) - 1),
            len,
        }
    }

    pub fn bits(&self) -> u16 {
        self.bits
    }

    pub fn len(&self) -> u8 {
        self.len
    }

    pub fn concat(self, other: Self) -> Self {
        if self.len + other.len > 16 {
            panic!("Too big sequences to concat");
        }
        let left_part = other.bits << self.len;
        Self {
            bits: left_part + self.bits,
            len: self.len + other.len,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

pub struct BitReader<T> {
    pub stream: T,
    buffer: BitSequence,
}

impl<T: BufRead> BitReader<T> {
    pub fn new(stream: T) -> Self {
        Self {
            stream,
            buffer: BitSequence { bits: 0, len: 0 },
        }
    }

    pub fn read_bits(&mut self, mut len: u8) -> io::Result<BitSequence> {
        let mut result = BitSequence::new(0, 0);
        while len > 0 {
            if len <= self.buffer.len() {
                let bits = self.buffer.bits() & ((1u16 << len) - 1);
                self.buffer.len -= len;
                self.buffer.bits >>= len;
                result = result.concat(BitSequence::new(bits, len));
                return Ok(result);
            }
            let byte = self.stream.read_u8()?;
            result = result.concat(self.buffer);
            len -= self.buffer.len();
            self.buffer = BitSequence::new(byte as u16, 8);
        }
        Ok(result)
    }

    pub fn borrow_reader_from_boundary(&mut self) -> &mut T {
        self.buffer = BitSequence::new(0, 0);
        &mut self.stream
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::ReadBytesExt;

    #[test]
    fn read_bits() -> io::Result<()> {
        let data: &[u8] = &[0b01100011, 0b11011011, 0b10101111];
        let mut reader = BitReader::new(data);
        assert_eq!(reader.read_bits(1)?, BitSequence::new(0b1, 1));
        assert_eq!(reader.read_bits(2)?, BitSequence::new(0b01, 2));
        assert_eq!(reader.read_bits(3)?, BitSequence::new(0b100, 3));
        assert_eq!(reader.read_bits(4)?, BitSequence::new(0b1101, 4));
        assert_eq!(reader.read_bits(5)?, BitSequence::new(0b10110, 5));
        assert_eq!(reader.read_bits(8)?, BitSequence::new(0b01011111, 8));
        assert_eq!(
            reader.read_bits(2).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        Ok(())
    }

    #[test]
    fn borrow_reader_from_boundary() -> io::Result<()> {
        let data: &[u8] = &[0b01100011, 0b11011011, 0b10101111];
        let mut reader = BitReader::new(data);
        assert_eq!(reader.read_bits(3)?, BitSequence::new(0b011, 3));
        assert_eq!(reader.borrow_reader_from_boundary().read_u8()?, 0b11011011);
        assert_eq!(reader.read_bits(8)?, BitSequence::new(0b10101111, 8));
        Ok(())
    }
}
