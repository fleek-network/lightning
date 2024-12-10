// Automatically generated rust module for 'unixfs.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy::all)]
#![cfg_attr(rustfmt, rustfmt_skip)]


use std::borrow::Cow;
use quick_protobuf::{MessageInfo, MessageRead, MessageWrite, BytesReader, Writer, WriterBackend, Result};
use quick_protobuf::sizeofs::*;
use super::*;

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Data<'a> {
    pub Type: mod_Data::DataType,
    pub Data: Cow<'a, [u8]>,
    pub filesize: u64,
    pub blocksizes: Vec<u64>,
    pub hashType: u64,
    pub fanout: u64,
}

impl<'a> Data<'a> {
    pub fn to_raw(bytes: &'a [u8]) -> Data<'a> {
        Data {
            Type: mod_Data::DataType::Raw,
            Data: Cow::Borrowed(bytes),
            filesize: bytes.len() as u64,
            blocksizes: vec![],
            hashType: 0,
            fanout: 0
        }
    }
}

impl<'a> MessageRead<'a> for Data<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.Type = r.read_enum(bytes)?,
                Ok(18) => msg.Data = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(24) => msg.filesize = r.read_uint64(bytes)?,
                Ok(34) => msg.blocksizes = r.read_packed(bytes, |r, bytes| Ok(r.read_uint64(bytes)?))?,
                Ok(40) => msg.hashType = r.read_uint64(bytes)?,
                Ok(48) => msg.fanout = r.read_uint64(bytes)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Data<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.Type == unixfs::mod_Data::DataType::Raw { 0 } else { 1 + sizeof_varint(*(&self.Type) as u64) }
        + if self.Data == Cow::Borrowed(b"") { 0 } else { 1 + sizeof_len((&self.Data).len()) }
        + if self.filesize == 0u64 { 0 } else { 1 + sizeof_varint(*(&self.filesize) as u64) }
        + if self.blocksizes.is_empty() { 0 } else { 1 + sizeof_len(self.blocksizes.iter().map(|s| sizeof_varint(*(s) as u64)).sum::<usize>()) }
        + if self.hashType == 0u64 { 0 } else { 1 + sizeof_varint(*(&self.hashType) as u64) }
        + if self.fanout == 0u64 { 0 } else { 1 + sizeof_varint(*(&self.fanout) as u64) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.Type != unixfs::mod_Data::DataType::Raw { w.write_with_tag(8, |w| w.write_enum(*&self.Type as i32))?; }
        if self.Data != Cow::Borrowed(b"") { w.write_with_tag(18, |w| w.write_bytes(&**&self.Data))?; }
        if self.filesize != 0u64 { w.write_with_tag(24, |w| w.write_uint64(*&self.filesize))?; }
        w.write_packed_with_tag(34, &self.blocksizes, |w, m| w.write_uint64(*m), &|m| sizeof_varint(*(m) as u64))?;
        if self.hashType != 0u64 { w.write_with_tag(40, |w| w.write_uint64(*&self.hashType))?; }
        if self.fanout != 0u64 { w.write_with_tag(48, |w| w.write_uint64(*&self.fanout))?; }
        Ok(())
    }
}

pub mod mod_Data {


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DataType {
    Raw = 0,
    Directory = 1,
    File = 2,
    Metadata = 3,
    Symlink = 4,
    HAMTShard = 5,
}

impl Default for DataType {
    fn default() -> Self {
        DataType::Raw
    }
}

impl From<i32> for DataType {
    fn from(i: i32) -> Self {
        match i {
            0 => DataType::Raw,
            1 => DataType::Directory,
            2 => DataType::File,
            3 => DataType::Metadata,
            4 => DataType::Symlink,
            5 => DataType::HAMTShard,
            _ => Self::default(),
        }
    }
}

impl<'a> From<&'a str> for DataType {
    fn from(s: &'a str) -> Self {
        match s {
            "Raw" => DataType::Raw,
            "Directory" => DataType::Directory,
            "File" => DataType::File,
            "Metadata" => DataType::Metadata,
            "Symlink" => DataType::Symlink,
            "HAMTShard" => DataType::HAMTShard,
            _ => Self::default(),
        }
    }
}

}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Metadata<'a> {
    pub MimeType: Cow<'a, str>,
}

impl<'a> MessageRead<'a> for Metadata<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.MimeType = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Metadata<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.MimeType == "" { 0 } else { 1 + sizeof_len((&self.MimeType).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.MimeType != "" { w.write_with_tag(10, |w| w.write_string(&**&self.MimeType))?; }
        Ok(())
    }
}

