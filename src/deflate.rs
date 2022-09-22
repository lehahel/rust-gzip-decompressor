#![forbid(unsafe_code)]

use std::io::BufRead;

use anyhow::{anyhow, Result};

use crate::bit_reader::BitReader;

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct BlockHeader {
    pub is_final: bool,
    pub compression_type: CompressionType,
}

#[derive(Debug, PartialEq)]
pub enum CompressionType {
    Uncompressed = 0,
    FixedTree = 1,
    DynamicTree = 2,
}

////////////////////////////////////////////////////////////////////////////////

pub struct DeflateReader<T> {
    bit_reader: BitReader<T>,
    data_left: bool,
}

impl<T: BufRead> DeflateReader<T> {
    pub fn new(bit_reader: BitReader<T>) -> Self {
        Self {
            bit_reader,
            data_left: true,
        }
    }

    // pub fn reader(&mut self) -> &mut BitReader<T> {
    //     &mut self.bit_reader
    // }

    pub fn next_block(&mut self) -> Option<Result<(BlockHeader, &mut BitReader<T>)>> {
        // println!("getting block header");
        if !self.data_left {
            return None;
        }
        match self.bit_reader.read_bits(1) {
            Ok(is_final_bits) => self.data_left = is_final_bits.bits() == 0,
            Err(err) => return Some(Err(anyhow::Error::new(err))),
        };
        let compression_type: CompressionType = match self.bit_reader.read_bits(2) {
            Ok(compression_type_bits) => match compression_type_bits.bits() {
                0 => {
                    // println!("got uncompressed");
                    CompressionType::Uncompressed
                }
                1 => {
                    // println!("got fixed tree");
                    CompressionType::FixedTree
                }
                2 => {
                    // println!("got dynamic tree");
                    CompressionType::DynamicTree
                }
                _ => {
                    // println!("unsupported block type");
                    return Some(Err(anyhow!("unsupported block type")));
                }
            },
            Err(err) => return Some(Err(anyhow::Error::new(err))),
        };
        // println!("got normal block type");
        Some(Ok((
            BlockHeader {
                is_final: !self.data_left,
                compression_type,
            },
            &mut self.bit_reader,
        )))
    }
}
