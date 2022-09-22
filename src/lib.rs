#![forbid(unsafe_code)]

use std::io::{BufRead, Write};

use anyhow::{bail, ensure, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use gzip::MemberReader;
use tracking_writer::TrackingWriter;

use crate::{
    // bit_reader::reverse_bits,
    bit_reader::BitReader,
    deflate::DeflateReader,
    gzip::{CompressionMethod, GzipReader},
    huffman_coding::{decode_litlen_distance_trees, get_fixed_tree, LitLenToken},
};

mod bit_reader;
mod deflate;
mod gzip;
mod huffman_coding;
mod tracking_writer;

pub fn decompress<R: BufRead, W: Write>(input: R, mut output: W) -> Result<()> {
    let mut gzip_reader = GzipReader::new(input);

    while let Some(member) = gzip_reader.read_header() {
        let mut writer = TrackingWriter::new(&mut output);
        let (header, _flags) = member?;
        if let CompressionMethod::Unknown(_) = header.compression_method {
            bail!("unsupported compression method")
        }

        let bit_reader = BitReader::new(gzip_reader.reader());
        let mut deflate_reader = DeflateReader::new(bit_reader);

        while let Some(block) = deflate_reader.next_block() {
            let (cur_header, cur_reader) = block?;
            if cur_header.compression_type == deflate::CompressionType::Uncompressed {
                // println!("processing uncompressed block");
                // cur_reader.read_bits(5)?;
                let len = cur_reader
                    .borrow_reader_from_boundary()
                    .read_u16::<LittleEndian>()?;
                let nlen = cur_reader
                    .borrow_reader_from_boundary()
                    .read_u16::<LittleEndian>()?;
                ensure!(len == !nlen, "nlen check failed");
                for _ in 0..len {
                    writer.write_all(&[cur_reader.borrow_reader_from_boundary().read_u8()?])?;
                }
                // println!("processed uncompressed block");
                continue;
            }
            let (litlen_tree, dist_tree) = match cur_header.compression_type {
                deflate::CompressionType::FixedTree => {
                    // println!("found fixed tree");
                    get_fixed_tree()?
                }
                deflate::CompressionType::DynamicTree => {
                    // println!("found dynamic tree");
                    decode_litlen_distance_trees(cur_reader)?
                }
                _ => bail!("should not occur"),
            };
            // println!("processing block");
            loop {
                match litlen_tree.read_symbol(cur_reader)? {
                    LitLenToken::Literal(byte) => {
                        // println!("writing literal: {}", byte);
                        writer.write_all(&[byte])?;
                    }
                    LitLenToken::Length { base, extra_bits } => {
                        // println!("writing length: ({}, {})", base, extra_bits);
                        // let len = base + reverse_bits(reader.read_bits(extra_bits)?.bits(), extra_bits);
                        let len = base + cur_reader.read_bits(extra_bits)?.bits();
                        // println!("  - got len: {}", len);
                        let dist_token = dist_tree.read_symbol(cur_reader)?;
                        // println!(
                        //     "  - dist token: base={} extra_bits={}",
                        //     dist_token.base, dist_token.extra_bits
                        // );
                        let dist =
                            dist_token.base + cur_reader.read_bits(dist_token.extra_bits)?.bits();
                        writer.write_previous(dist as usize, len as usize)?;
                    }
                    LitLenToken::EndOfBlock => {
                        // println!("reached end of block");
                        break;
                    }
                };
            }
        }

        let member_reader = MemberReader::new(gzip_reader.reader());
        let (footer, _reader) = member_reader.read_footer()?;

        if footer.data_size as usize != writer.byte_count() {
            bail!("length check failed");
        }

        if footer.data_crc32 != writer.crc32() {
            bail!("crc32 check failed");
        }
    }
    Ok(())
}
