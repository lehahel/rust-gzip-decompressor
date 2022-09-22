#![forbid(unsafe_code)]

use std::{collections::HashMap, convert::TryFrom, io::BufRead};

use anyhow::{anyhow, ensure, Result};

use crate::bit_reader::{BitReader, BitSequence};

////////////////////////////////////////////////////////////////////////////////

pub fn get_fixed_tree() -> Result<(HuffmanCoding<LitLenToken>, HuffmanCoding<DistanceToken>)> {
    let mut lengths = vec![];
    for _i in 0..=143 {
        lengths.push(8);
    }
    for _i in 144..=255 {
        lengths.push(9);
    }
    for _i in 256..=279 {
        lengths.push(7);
    }
    for _i in 280..=287 {
        lengths.push(8);
    }
    let dists = vec![5; 32];
    let litlen_tree = HuffmanCoding::<LitLenToken>::from_lengths(lengths.as_slice())?;
    let dist_tree = HuffmanCoding::<DistanceToken>::from_lengths(dists.as_slice())?;
    Ok((litlen_tree, dist_tree))
}

pub fn decode_litlen_distance_trees<T: BufRead>(
    bit_reader: &mut BitReader<T>,
) -> Result<(HuffmanCoding<LitLenToken>, HuffmanCoding<DistanceToken>)> {
    let hlit = bit_reader.read_bits(5)?.bits() as usize + 257;
    let hdist = bit_reader.read_bits(5)?.bits() as usize + 1;
    let hclen = bit_reader.read_bits(4)?.bits() as usize + 4;

    let lengths_map: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut bl_tree: [u8; 19] = [0; 19];

    for i in 0usize..hclen {
        bl_tree[lengths_map[i]] = bit_reader.read_bits(3)?.bits() as u8;
    }
    let mapper = HuffmanCoding::<TreeCodeToken>::from_lengths(&bl_tree)?;
    let mut tokens = Vec::<u8>::new();
    while tokens.len() < hlit + hdist {
        let symbol = mapper.read_symbol(bit_reader)?;
        match symbol {
            TreeCodeToken::Length(value) => tokens.push(value),
            TreeCodeToken::CopyPrev => {
                ensure!(!tokens.is_empty(), "invalid tree");
                let repeat_count = bit_reader.read_bits(2)?.bits() as usize + 3;
                tokens.resize(tokens.len() + repeat_count, *tokens.last().unwrap());
            }
            TreeCodeToken::RepeatZero { base, extra_bits } => {
                let repeat_count = bit_reader.read_bits(extra_bits)?.bits() + base;
                tokens.resize(tokens.len() + repeat_count as usize, 0);
            }
        };
    }
    let litlen_tree = HuffmanCoding::<LitLenToken>::from_lengths(&tokens[..hlit])?;

    let potential_dist_tree = &tokens[hlit..];
    let mut count_one_length = 0usize;
    let mut count_positive_length = 0usize;

    for &item in potential_dist_tree {
        match item {
            0 => {}
            1 => {
                count_one_length += 1;
            }
            _ => {
                count_positive_length += 1;
            }
        }
    }

    let mut lengths_bad = vec![0u8, 31];
    lengths_bad.push(1u8);

    let lengths = match (count_one_length, count_positive_length) {
        (1, 0) => &lengths_bad,
        _ => potential_dist_tree,
    };

    let dist_tree = HuffmanCoding::<DistanceToken>::from_lengths(lengths)?;
    Ok((litlen_tree, dist_tree))
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug)]
pub enum TreeCodeToken {
    Length(u8),
    CopyPrev,
    RepeatZero { base: u16, extra_bits: u8 },
}

impl TryFrom<HuffmanCodeWord> for TreeCodeToken {
    type Error = anyhow::Error;

    fn try_from(value: HuffmanCodeWord) -> Result<Self> {
        match value.0 {
            value if value <= 15 => Ok(TreeCodeToken::Length(value as u8)),
            16 => Ok(TreeCodeToken::CopyPrev),
            17 => Ok(TreeCodeToken::RepeatZero {
                base: 3,
                extra_bits: 3,
            }),
            18 => Ok(TreeCodeToken::RepeatZero {
                base: 11,
                extra_bits: 7,
            }),
            _ => Err(anyhow!("invalid value for TreeCodeToken")),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug)]
pub enum LitLenToken {
    Literal(u8),
    EndOfBlock,
    Length { base: u16, extra_bits: u8 },
}

impl TryFrom<HuffmanCodeWord> for LitLenToken {
    type Error = anyhow::Error;

    fn try_from(value: HuffmanCodeWord) -> Result<Self> {
        match value.0 {
            value if value < 256 => Ok(LitLenToken::Literal(value as u8)),
            256 => Ok(LitLenToken::EndOfBlock),
            257 => Ok(LitLenToken::Length {
                base: 3,
                extra_bits: 0,
            }),
            258 => Ok(LitLenToken::Length {
                base: 4,
                extra_bits: 0,
            }),
            259 => Ok(LitLenToken::Length {
                base: 5,
                extra_bits: 0,
            }),
            260 => Ok(LitLenToken::Length {
                base: 6,
                extra_bits: 0,
            }),
            261 => Ok(LitLenToken::Length {
                base: 7,
                extra_bits: 0,
            }),
            262 => Ok(LitLenToken::Length {
                base: 8,
                extra_bits: 0,
            }),
            263 => Ok(LitLenToken::Length {
                base: 9,
                extra_bits: 0,
            }),
            264 => Ok(LitLenToken::Length {
                base: 10,
                extra_bits: 0,
            }),
            265 => Ok(LitLenToken::Length {
                base: 11,
                extra_bits: 1,
            }),
            266 => Ok(LitLenToken::Length {
                base: 13,
                extra_bits: 1,
            }),
            267 => Ok(LitLenToken::Length {
                base: 15,
                extra_bits: 1,
            }),
            268 => Ok(LitLenToken::Length {
                base: 17,
                extra_bits: 1,
            }),
            269 => Ok(LitLenToken::Length {
                base: 19,
                extra_bits: 2,
            }),
            270 => Ok(LitLenToken::Length {
                base: 23,
                extra_bits: 2,
            }),
            271 => Ok(LitLenToken::Length {
                base: 27,
                extra_bits: 2,
            }),
            272 => Ok(LitLenToken::Length {
                base: 31,
                extra_bits: 2,
            }),
            273 => Ok(LitLenToken::Length {
                base: 35,
                extra_bits: 3,
            }),
            274 => Ok(LitLenToken::Length {
                base: 43,
                extra_bits: 3,
            }),
            275 => Ok(LitLenToken::Length {
                base: 51,
                extra_bits: 3,
            }),
            276 => Ok(LitLenToken::Length {
                base: 59,
                extra_bits: 3,
            }),
            277 => Ok(LitLenToken::Length {
                base: 67,
                extra_bits: 4,
            }),
            278 => Ok(LitLenToken::Length {
                base: 83,
                extra_bits: 4,
            }),
            279 => Ok(LitLenToken::Length {
                base: 99,
                extra_bits: 4,
            }),
            280 => Ok(LitLenToken::Length {
                base: 115,
                extra_bits: 4,
            }),
            281 => Ok(LitLenToken::Length {
                base: 131,
                extra_bits: 5,
            }),
            282 => Ok(LitLenToken::Length {
                base: 163,
                extra_bits: 5,
            }),
            283 => Ok(LitLenToken::Length {
                base: 195,
                extra_bits: 5,
            }),
            284 => Ok(LitLenToken::Length {
                base: 227,
                extra_bits: 5,
            }),
            285 => Ok(LitLenToken::Length {
                base: 258,
                extra_bits: 0,
            }),
            _ => Err(anyhow!("incorrect value for LitLenToken")),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug)]
pub struct DistanceToken {
    pub base: u16,
    pub extra_bits: u8,
}

impl TryFrom<HuffmanCodeWord> for DistanceToken {
    type Error = anyhow::Error;

    fn try_from(value: HuffmanCodeWord) -> Result<Self> {
        match value.0 {
            0 => Ok(DistanceToken {
                base: 1,
                extra_bits: 0,
            }),
            1 => Ok(DistanceToken {
                base: 2,
                extra_bits: 0,
            }),
            2 => Ok(DistanceToken {
                base: 3,
                extra_bits: 0,
            }),
            3 => Ok(DistanceToken {
                base: 4,
                extra_bits: 0,
            }),
            4 => Ok(DistanceToken {
                base: 5,
                extra_bits: 1,
            }),
            5 => Ok(DistanceToken {
                base: 7,
                extra_bits: 1,
            }),
            6 => Ok(DistanceToken {
                base: 9,
                extra_bits: 2,
            }),
            7 => Ok(DistanceToken {
                base: 13,
                extra_bits: 2,
            }),
            8 => Ok(DistanceToken {
                base: 17,
                extra_bits: 3,
            }),
            9 => Ok(DistanceToken {
                base: 25,
                extra_bits: 3,
            }),
            10 => Ok(DistanceToken {
                base: 33,
                extra_bits: 4,
            }),
            11 => Ok(DistanceToken {
                base: 49,
                extra_bits: 4,
            }),
            12 => Ok(DistanceToken {
                base: 65,
                extra_bits: 5,
            }),
            13 => Ok(DistanceToken {
                base: 97,
                extra_bits: 5,
            }),
            14 => Ok(DistanceToken {
                base: 129,
                extra_bits: 6,
            }),
            15 => Ok(DistanceToken {
                base: 193,
                extra_bits: 6,
            }),
            16 => Ok(DistanceToken {
                base: 257,
                extra_bits: 7,
            }),
            17 => Ok(DistanceToken {
                base: 385,
                extra_bits: 7,
            }),
            18 => Ok(DistanceToken {
                base: 513,
                extra_bits: 8,
            }),
            19 => Ok(DistanceToken {
                base: 769,
                extra_bits: 8,
            }),
            20 => Ok(DistanceToken {
                base: 1025,
                extra_bits: 9,
            }),
            21 => Ok(DistanceToken {
                base: 1537,
                extra_bits: 9,
            }),
            22 => Ok(DistanceToken {
                base: 2049,
                extra_bits: 10,
            }),
            23 => Ok(DistanceToken {
                base: 3073,
                extra_bits: 10,
            }),
            24 => Ok(DistanceToken {
                base: 4097,
                extra_bits: 11,
            }),
            25 => Ok(DistanceToken {
                base: 6145,
                extra_bits: 11,
            }),
            26 => Ok(DistanceToken {
                base: 8193,
                extra_bits: 12,
            }),
            27 => Ok(DistanceToken {
                base: 12289,
                extra_bits: 12,
            }),
            28 => Ok(DistanceToken {
                base: 16385,
                extra_bits: 13,
            }),
            29 => Ok(DistanceToken {
                base: 24577,
                extra_bits: 13,
            }),
            _ => Err(anyhow!("incorrect token value")),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

const MAX_BITS: usize = 15;

pub struct HuffmanCodeWord(pub u16);

pub struct HuffmanCoding<T> {
    map: HashMap<BitSequence, T>,
}

impl<T> HuffmanCoding<T>
where
    T: Copy + TryFrom<HuffmanCodeWord, Error = anyhow::Error>,
{
    // pub fn new(map: HashMap<BitSequence, T>) -> Self {
    //     Self { map }
    // }

    #[allow(unused)]
    pub fn decode_symbol(&self, seq: BitSequence) -> Option<T> {
        self.map.get(&seq).copied()
    }

    pub fn read_symbol<U: BufRead>(&self, bit_reader: &mut BitReader<U>) -> Result<T> {
        let mut current = BitSequence::new(0, 0);
        for _ in 1usize..=MAX_BITS {
            let bit = bit_reader.read_bits(1)?;
            current = bit.concat(current);
            match self.map.get(&current) {
                Some(val_ref) => return Ok(*val_ref),
                None => continue,
            }
        }
        Err(anyhow!("undefined symbol"))
    }

    pub fn from_lengths(code_lengths: &[u8]) -> Result<Self> {
        // println!("getting lengths count");
        let mut bl_count = vec![0; MAX_BITS + 1];
        for code_length in code_lengths {
            bl_count[*code_length as usize] += 1;
        }
        // println!("getting next codes");
        bl_count[0] = 0;
        let mut code = 0;
        let mut next_code = vec![0; MAX_BITS + 1];
        for bits in 1usize..=MAX_BITS {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }
        // println!("getting tree");
        let mut map = HashMap::<BitSequence, T>::new();
        for code in 0u16..code_lengths.len() as u16 {
            let len = code_lengths[code as usize] as usize;
            if len != 0 {
                // println!("inserting {:#b} {} ({})", next_code[len], code, len);
                map.insert(
                    BitSequence::new(next_code[len], len as u8),
                    match T::try_from(HuffmanCodeWord(code)) {
                        Ok(val) => val,
                        _ => continue,
                    },
                );
                next_code[len] += 1;
            }
        }
        // println!("got tree");
        Ok(Self { map })
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Value(u16);

    impl TryFrom<HuffmanCodeWord> for Value {
        type Error = anyhow::Error;

        fn try_from(x: HuffmanCodeWord) -> Result<Self> {
            Ok(Self(x.0))
        }
    }

    #[test]
    fn from_lengths() -> Result<()> {
        let code = HuffmanCoding::<Value>::from_lengths(&[2, 3, 4, 3, 3, 4, 2])?;

        assert_eq!(
            code.decode_symbol(BitSequence::new(0b00, 2)),
            Some(Value(0)),
        );
        assert_eq!(
            code.decode_symbol(BitSequence::new(0b100, 3)),
            Some(Value(1)),
        );
        assert_eq!(
            code.decode_symbol(BitSequence::new(0b1110, 4)),
            Some(Value(2)),
        );
        assert_eq!(
            code.decode_symbol(BitSequence::new(0b101, 3)),
            Some(Value(3)),
        );
        assert_eq!(
            code.decode_symbol(BitSequence::new(0b110, 3)),
            Some(Value(4)),
        );
        assert_eq!(
            code.decode_symbol(BitSequence::new(0b1111, 4)),
            Some(Value(5)),
        );
        assert_eq!(
            code.decode_symbol(BitSequence::new(0b01, 2)),
            Some(Value(6)),
        );

        assert_eq!(code.decode_symbol(BitSequence::new(0b0, 1)), None);
        assert_eq!(code.decode_symbol(BitSequence::new(0b10, 2)), None);
        assert_eq!(code.decode_symbol(BitSequence::new(0b111, 3)), None,);

        Ok(())
    }

    #[test]
    fn read_symbol() -> Result<()> {
        let code = HuffmanCoding::<Value>::from_lengths(&[2, 3, 4, 3, 3, 4, 2])?;
        let mut data: &[u8] = &[0b10111001, 0b11001010, 0b11101101];
        let mut reader = BitReader::new(&mut data);

        assert_eq!(code.read_symbol(&mut reader)?, Value(1));
        assert_eq!(code.read_symbol(&mut reader)?, Value(2));
        assert_eq!(code.read_symbol(&mut reader)?, Value(3));
        assert_eq!(code.read_symbol(&mut reader)?, Value(6));
        assert_eq!(code.read_symbol(&mut reader)?, Value(0));
        assert_eq!(code.read_symbol(&mut reader)?, Value(2));
        assert_eq!(code.read_symbol(&mut reader)?, Value(4));
        assert!(code.read_symbol(&mut reader).is_err());

        Ok(())
    }

    #[test]
    fn from_lengths_with_zeros() -> Result<()> {
        let lengths = [3, 4, 5, 5, 0, 0, 6, 6, 4, 0, 6, 0, 7];
        let code = HuffmanCoding::<Value>::from_lengths(&lengths)?;
        let mut data: &[u8] = &[
            0b00100000, 0b00100001, 0b00010101, 0b10010101, 0b00110101, 0b00011101,
        ];
        let mut reader = BitReader::new(&mut data);

        assert_eq!(code.read_symbol(&mut reader)?, Value(0));
        assert_eq!(code.read_symbol(&mut reader)?, Value(1));
        assert_eq!(code.read_symbol(&mut reader)?, Value(2));
        assert_eq!(code.read_symbol(&mut reader)?, Value(3));
        assert_eq!(code.read_symbol(&mut reader)?, Value(6));
        assert_eq!(code.read_symbol(&mut reader)?, Value(7));
        assert_eq!(code.read_symbol(&mut reader)?, Value(8));
        assert_eq!(code.read_symbol(&mut reader)?, Value(10));
        assert_eq!(code.read_symbol(&mut reader)?, Value(12));
        assert!(code.read_symbol(&mut reader).is_err());

        Ok(())
    }

    #[test]
    fn from_lengths_additional() -> Result<()> {
        let lengths = [
            9, 10, 10, 8, 8, 8, 5, 6, 4, 5, 4, 5, 4, 5, 4, 4, 5, 4, 4, 5, 4, 5, 4, 5, 5, 5, 4, 6, 6,
        ];
        let code = HuffmanCoding::<Value>::from_lengths(&lengths)?;
        let mut data: &[u8] = &[
            0b11111000, 0b10111100, 0b01010001, 0b11111111, 0b00110101, 0b11111001, 0b11011111,
            0b11100001, 0b01110111, 0b10011111, 0b10111111, 0b00110100, 0b10111010, 0b11111111,
            0b11111101, 0b10010100, 0b11001110, 0b01000011, 0b11100111, 0b00000010,
        ];
        let mut reader = BitReader::new(&mut data);

        assert_eq!(code.read_symbol(&mut reader)?, Value(10));
        assert_eq!(code.read_symbol(&mut reader)?, Value(7));
        assert_eq!(code.read_symbol(&mut reader)?, Value(27));
        assert_eq!(code.read_symbol(&mut reader)?, Value(22));
        assert_eq!(code.read_symbol(&mut reader)?, Value(9));
        assert_eq!(code.read_symbol(&mut reader)?, Value(0));
        assert_eq!(code.read_symbol(&mut reader)?, Value(11));
        assert_eq!(code.read_symbol(&mut reader)?, Value(15));
        assert_eq!(code.read_symbol(&mut reader)?, Value(2));
        assert_eq!(code.read_symbol(&mut reader)?, Value(20));
        assert_eq!(code.read_symbol(&mut reader)?, Value(8));
        assert_eq!(code.read_symbol(&mut reader)?, Value(4));
        assert_eq!(code.read_symbol(&mut reader)?, Value(23));
        assert_eq!(code.read_symbol(&mut reader)?, Value(24));
        assert_eq!(code.read_symbol(&mut reader)?, Value(5));
        assert_eq!(code.read_symbol(&mut reader)?, Value(26));
        assert_eq!(code.read_symbol(&mut reader)?, Value(18));
        assert_eq!(code.read_symbol(&mut reader)?, Value(12));
        assert_eq!(code.read_symbol(&mut reader)?, Value(25));
        assert_eq!(code.read_symbol(&mut reader)?, Value(1));
        assert_eq!(code.read_symbol(&mut reader)?, Value(3));
        assert_eq!(code.read_symbol(&mut reader)?, Value(6));
        assert_eq!(code.read_symbol(&mut reader)?, Value(13));
        assert_eq!(code.read_symbol(&mut reader)?, Value(14));
        assert_eq!(code.read_symbol(&mut reader)?, Value(16));
        assert_eq!(code.read_symbol(&mut reader)?, Value(17));
        assert_eq!(code.read_symbol(&mut reader)?, Value(19));
        assert_eq!(code.read_symbol(&mut reader)?, Value(21));

        Ok(())
    }
}
