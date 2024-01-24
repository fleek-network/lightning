mod proto;

use std::io;

pub use proto::unixfs::mod_Data::DataType;
pub use proto::unixfs::{Data, Metadata};

impl<'a> TryFrom<&'a [u8]> for Data<'a> {
    type Error = io::Error;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        use quick_protobuf::{BytesReader, MessageRead};
        Data::from_reader(&mut BytesReader::from_bytes(data), data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
