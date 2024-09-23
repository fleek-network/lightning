//! This module  provides a an abstraction to read and decode IPFS data.
//!
//! The abstractions written here allows to write or use any of the following use cases:
//!
//! - Decode bytes into any of the IPLD spec formats such as DAG-PB (Protocol Buffers), DAG-CBOR
//!   (Concise Binary Object Representation), etc. In those cases it is encourage to use any of the
//!   existing codec implementation in `ipld` Rust libraries.
//! - Read the decoded IPLD format `data` field and decode it into a specific user format, for
//!   example `Data` (UnixFS).
//!
//! On any doubts or question please refer to the [IPLD Spec](https://ipld.io/specs/about/).
use std::path::PathBuf;

use bytes::Bytes;
use ipld_core::codec::Codec;
use ipld_dagpb::{DagPbCodec, PbNode};
use multihash::{Code, MultihashDigest};

use super::fs::{DocId, IpldItem, Link};
use crate::errors::IpldError;
use crate::unixfs::{Data, DataType};

/// Trait to decode the `data` field from a IPLD abstract format `F`.
pub trait DataCodec<F> {
    fn decode_from(doc_id: &DocId, data: F) -> Result<IpldItem, IpldError>;
}

/// Trait to decode a IPLD abstract format `Ipld` into a specific user format p `DataCodec`.
/// Note that the use of Associated Types is encouraged to allow the user to define the specific
/// types for the IPLD abstract format and the specific user format, because the decoding is a
/// 2-step process:
///
/// 1. Decode the IPLD abstract format DAG-PB, DAG-CBOR, etc.
/// 2. Decode the `data` field from the IPLD abstract format into a specific user format, like
///    UnixFS.
pub trait Decoder {
    /// The IPLD abstract format.
    type Ipld;

    /// The error type for the IPLD abstract format.
    type Err: std::fmt::Debug;

    /// The codec to decode the IPLD abstract format which is constraint to the `Codec` trait in
    /// the `ipld_core` library.
    type IpldCodec: Codec<Self::Ipld, Error = Self::Err>;

    /// The codec to decode the `data` field from the IPLD abstract format into a specific user
    type DataCodec: DataCodec<Self::Ipld>;

    /// Get Bytes from the IPLD abstract format
    fn get_data(data: &Self::Ipld) -> Result<Option<Bytes>, IpldError>;

    /// Perform the 2 step decoding process.
    fn decode_from_slice(&self, doc_id: &DocId, data: &[u8]) -> Result<IpldItem, IpldError> {
        let node = Self::IpldCodec::decode_from_slice(data)
            .map_err(|e| IpldError::IpldCodecError(*doc_id.cid(), format!("{e:?}")))?;
        Self::validate_data(doc_id, data)?;
        Self::DataCodec::decode_from(doc_id, node)
    }

    fn validate_data(doc_id: &DocId, data: &[u8]) -> Result<(), IpldError> {
        if let Ok(hasher) = Code::try_from(doc_id.cid().hash().code()) {
            if hasher.digest(data).digest() == doc_id.cid().hash().digest() {
                Ok(())
            } else {
                Err(IpldError::MultihashError(*doc_id.cid()))
            }
        } else {
            Err(IpldError::MultihashCodeError(doc_id.cid().hash().code()))
        }
    }
}

/// Default implementation for `UnixFs`.
///
/// `DagPbCodec` ---> `Data` (UnixFs) ---> `IpldItem`
pub struct UnixFsProtobufCodec;

impl UnixFsProtobufCodec {
    fn from_result(
        id: &mut DocId,
        previous_path: Option<&PathBuf>,
        node: &PbNode,
        data: &Data,
    ) -> Result<IpldItem, IpldError> {
        match data.Type {
            DataType::Directory => {
                id.merge(previous_path, None);
                let links = Link::get_links(&node.links);
                Ok(IpldItem::to_dir(id.clone(), links))
            },
            DataType::File => {
                if node.links.is_empty() {
                    let data = Bytes::copy_from_slice(&data.Data);
                    Ok(IpldItem::to_file(id.clone(), data))
                } else {
                    Ok(IpldItem::to_chunked_file(
                        id.clone(),
                        Link::get_links(&node.links),
                    ))
                }
            },
            _ => Err(IpldError::UnsupportedUnixFsDataType(*id.cid())),
        }
    }
}

impl DataCodec<PbNode> for UnixFsProtobufCodec {
    fn decode_from(doc_id: &DocId, node: PbNode) -> Result<IpldItem, IpldError> {
        if node.data.is_none() {
            return Err(IpldError::UnixFsDecodingError(
                "No Data. Not UnixFs".to_string(),
            ));
        }
        let data = Data::try_from(&node.data)?;
        let mut doc_id = doc_id.clone();
        Self::from_result(&mut doc_id, None, &node, &data)
    }
}

/// Default implementation for the combination of `DagPbCodec` and `UnixFs`.
/// This is the most common use case for IPFS data.
///
/// `Bytes` ---> `DagPbCodec` ---> `Data` (UnixFs) ---> `IpldItem`
#[derive(Default, Clone)]
pub struct DagPbWithUnixFsCodec;

pub type DefaultDecoder = DagPbWithUnixFsCodec;

impl Decoder for DagPbWithUnixFsCodec {
    type Err = ipld_dagpb::Error;
    type Ipld = PbNode;
    type IpldCodec = DagPbCodec;
    type DataCodec = UnixFsProtobufCodec;

    fn get_data(node: &PbNode) -> Result<Option<Bytes>, IpldError> {
        Ok(node.data.clone())
    }
}
