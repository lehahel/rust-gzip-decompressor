#![forbid(unsafe_code)]

use std::io::BufRead;

use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use crc::Crc;

////////////////////////////////////////////////////////////////////////////////

const ID1: u8 = 0x1f;
const ID2: u8 = 0x8b;

const CM_DEFLATE: u8 = 8;

const FTEXT_OFFSET: u8 = 0;
const FHCRC_OFFSET: u8 = 1;
const FEXTRA_OFFSET: u8 = 2;
const FNAME_OFFSET: u8 = 3;
const FCOMMENT_OFFSET: u8 = 4;

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MemberHeader {
    pub compression_method: CompressionMethod,
    pub modification_time: u32,
    pub extra: Option<Vec<u8>>,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub extra_flags: u8,
    pub os: u8,
    pub has_crc: bool,
    pub is_text: bool,
}

impl MemberHeader {
    pub fn crc16(&self) -> u16 {
        let crc = Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
        let mut digest = crc.digest();

        digest.update(&[ID1, ID2, self.compression_method.into(), self.flags().0]);
        digest.update(&self.modification_time.to_le_bytes());
        digest.update(&[self.extra_flags, self.os]);

        if let Some(extra) = &self.extra {
            digest.update(&(extra.len() as u16).to_le_bytes());
            digest.update(extra);
        }

        if let Some(name) = &self.name {
            digest.update(name.as_bytes());
            digest.update(&[0]);
        }

        if let Some(comment) = &self.comment {
            digest.update(comment.as_bytes());
            digest.update(&[0]);
        }

        (digest.finalize() & 0xffff) as u16
    }

    pub fn flags(&self) -> MemberFlags {
        let mut flags = MemberFlags(0);
        flags.set_is_text(self.is_text);
        flags.set_has_crc(self.has_crc);
        flags.set_has_extra(self.extra.is_some());
        flags.set_has_name(self.name.is_some());
        flags.set_has_comment(self.comment.is_some());
        flags
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug)]
pub enum CompressionMethod {
    Deflate,
    Unknown(u8),
}

impl From<u8> for CompressionMethod {
    fn from(value: u8) -> Self {
        match value {
            CM_DEFLATE => Self::Deflate,
            x => Self::Unknown(x),
        }
    }
}

impl From<CompressionMethod> for u8 {
    fn from(method: CompressionMethod) -> u8 {
        match method {
            CompressionMethod::Deflate => CM_DEFLATE,
            CompressionMethod::Unknown(x) => x,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MemberFlags(u8);

#[allow(unused)]
impl MemberFlags {
    fn bit(&self, n: u8) -> bool {
        (self.0 >> n) & 1 != 0
    }

    fn set_bit(&mut self, n: u8, value: bool) {
        if value {
            self.0 |= 1 << n;
        } else {
            self.0 &= !(1 << n);
        }
    }

    pub fn is_text(&self) -> bool {
        self.bit(FTEXT_OFFSET)
    }

    pub fn set_is_text(&mut self, value: bool) {
        self.set_bit(FTEXT_OFFSET, value)
    }

    pub fn has_crc(&self) -> bool {
        self.bit(FHCRC_OFFSET)
    }

    pub fn set_has_crc(&mut self, value: bool) {
        self.set_bit(FHCRC_OFFSET, value)
    }

    pub fn has_extra(&self) -> bool {
        self.bit(FEXTRA_OFFSET)
    }

    pub fn set_has_extra(&mut self, value: bool) {
        self.set_bit(FEXTRA_OFFSET, value)
    }

    pub fn has_name(&self) -> bool {
        self.bit(FNAME_OFFSET)
    }

    pub fn set_has_name(&mut self, value: bool) {
        self.set_bit(FNAME_OFFSET, value)
    }

    pub fn has_comment(&self) -> bool {
        self.bit(FCOMMENT_OFFSET)
    }

    pub fn set_has_comment(&mut self, value: bool) {
        self.set_bit(FCOMMENT_OFFSET, value)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MemberFooter {
    pub data_crc32: u32,
    pub data_size: u32,
}

////////////////////////////////////////////////////////////////////////////////

pub struct GzipReader<T> {
    reader: T,
}

impl<T: BufRead> GzipReader<T> {
    pub fn new(reader: T) -> Self {
        Self { reader }
    }

    pub fn reader(&mut self) -> &mut T {
        &mut self.reader
    }

    fn read_string(&mut self) -> Result<String> {
        let mut buffer = vec![];
        self.reader.read_until(0, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }

    pub fn read_header(&mut self) -> Option<Result<(MemberHeader, MemberFlags)>> {
        let id1 = match self.reader.read_u8() {
            Ok(ok) => ok,
            _ => return None,
        };
        let id2 = match self.reader.read_u8() {
            Err(err) => return Some(Err(anyhow!(err))),
            Ok(ok) => ok,
        };
        if id1 != 31 || id2 != 139 {
            return Some(Err(anyhow!("wrong id values")));
        }
        let compression_method = CompressionMethod::from(match self.reader.read_u8() {
            Ok(ok) => ok,
            Err(err) => return Some(Err(anyhow!(err))),
        });
        let member_flags = MemberFlags(self.reader.read_u8().unwrap());
        let modification_time = self.reader.read_u32::<LittleEndian>().unwrap();
        let extra_flags = self.reader.read_u8().unwrap();
        let os = self.reader.read_u8().unwrap();
        let mut extra = None;
        if member_flags.has_extra() {
            // self.reader.read_u16::<LittleEndian>().ok()?;
            let extra_len = self.reader.read_u16::<LittleEndian>().unwrap();
            let mut buffer = vec![0; extra_len as usize];
            self.reader.read_exact(buffer.as_mut_slice()).unwrap();
            extra = Some(buffer);
        }
        let name = match member_flags.has_name() {
            true => Some(self.read_string().unwrap()),
            false => None,
        };
        let comment = match member_flags.has_comment() {
            true => Some(self.read_string().unwrap()),
            false => None,
        };
        let has_crc = member_flags.has_crc();
        let is_text = member_flags.is_text();

        let member_header = MemberHeader {
            compression_method,
            modification_time,
            extra,
            name,
            comment,
            extra_flags,
            os,
            has_crc,
            is_text,
        };

        if has_crc && self.reader.read_u16::<LittleEndian>().ok()? != member_header.crc16() {
            return Some(Err(anyhow!("header crc16 check failed")));
        }
        Some(Ok((member_header, member_flags)))
    }
}

////////////////////////////////////////////////////////////////////////////////

pub struct MemberReader<T> {
    inner: T,
}

impl<T: BufRead> MemberReader<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    // pub fn inner_mut(&mut self) -> &mut T {
    //     &mut self.inner
    // }

    pub fn read_footer(mut self) -> Result<(MemberFooter, GzipReader<T>)> {
        let data_crc32 = self.inner.read_u32::<LittleEndian>()?;
        let data_size = self.inner.read_u32::<LittleEndian>()?;
        Ok((
            MemberFooter {
                data_crc32,
                data_size,
            },
            GzipReader::new(self.inner),
        ))
    }
}
