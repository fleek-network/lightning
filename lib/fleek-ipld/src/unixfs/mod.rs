mod proto;

use bytes::Bytes;
pub use proto::unixfs::mod_Data::DataType;
pub use proto::unixfs::{Data, Metadata};

use crate::errors::IpldError;

impl<'a> TryFrom<&'a [u8]> for Data<'a> {
    type Error = IpldError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        use quick_protobuf::{BytesReader, MessageRead};
        Data::from_reader(&mut BytesReader::from_bytes(data), data)
            .map_err(|e| IpldError::UnixFsDecodingError(e.to_string()))
    }
}

impl<'a> TryFrom<&'a Option<Bytes>> for Data<'a> {
    type Error = IpldError;

    fn try_from(data: &'a Option<Bytes>) -> Result<Self, Self::Error> {
        match data {
            Some(data) => {
                let data = data.as_ref();
                Self::try_from(data)
            },
            None => Err(IpldError::UnixFsDecodingError(
                "No data found converting from PbNode".to_string(),
            )),
        }
    }
}
