use std::str::FromStr;

use async_trait::async_trait;
use ipld_core::cid::Cid;

use self::errors::IpldError;

pub mod errors;

pub struct DocId(Cid);

impl From<Cid> for DocId {
    fn from(cid: Cid) -> Self {
        Self(cid)
    }
}

impl FromStr for DocId {
    type Err = IpldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Cid::from_str(s)?))
    }
}

pub struct DocDir {
    pub id: DocId,
    pub name: String,
}

pub struct DocFile {
    pub id: DocId,
    pub name: String,
    pub folder: DocId,
    pub data: Vec<u8>,
    pub size: u64,
}

pub enum IpldItem {
    Dir(DocDir),
    File(DocFile),
}

#[async_trait]
pub trait Processor {
    type Codec;

    async fn process(&self, id: DocId) -> Result<(), IpldError>;
}
