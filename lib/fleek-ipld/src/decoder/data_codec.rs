use std::path::PathBuf;

use bytes::Bytes;
use ipld_core::codec::Codec;
use ipld_dagpb::{DagPbCodec, PbNode};

use super::fs::{DocId, IpldItem, Link};
use crate::errors::IpldError;
use crate::unixfs::{Data, DataType};

pub trait DataCodec<F> {
    fn decode_from(doc_id: &DocId, data: F) -> Result<IpldItem, IpldError>;
}

pub trait Decoder {
    type Ipld;

    type IpldCodec: Codec<Self::Ipld>;

    type DataCodec: DataCodec<Self::Ipld>;

    fn get_data(data: &Self::Ipld) -> Result<Option<Bytes>, IpldError>;

    fn decode_from_slice(&self, doc_id: &DocId, data: &[u8]) -> Result<IpldItem, IpldError> {
        let node =
            Self::IpldCodec::decode_from_slice(data).map_err(|_| IpldError::IpldCodecError)?;
        Self::DataCodec::decode_from(doc_id, node)
    }
}

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
                id.merge(previous_path);
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
            _ => Err(IpldError::UnsupportedUnixFsDataType(format!(
                "{:?} - {:?}",
                id, data.Type
            ))),
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

#[derive(Default)]
pub struct DagPbWithUnixFsCodec;

pub type DefaultDecoder = DagPbWithUnixFsCodec;

impl Decoder for DagPbWithUnixFsCodec {
    type Ipld = PbNode;
    type IpldCodec = DagPbCodec;
    type DataCodec = UnixFsProtobufCodec;

    fn get_data(node: &PbNode) -> Result<Option<Bytes>, IpldError> {
        Ok(node.data.clone())
    }
}
