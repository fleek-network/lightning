use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::Blake3Hash;

use crate::Config;

pub struct B3FSOrigin<C: Collection> {
    _phantom: std::marker::PhantomData<C>,
}

impl<C: Collection> Clone for B3FSOrigin<C> {
    fn clone(&self) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<C: Collection> B3FSOrigin<C> {
    pub fn new(_config: Config) -> Result<Self> {
        Ok(B3FSOrigin {
            _phantom: std::marker::PhantomData,
        })
    }

    pub async fn fetch(&self, _uri: &[u8]) -> Result<Blake3Hash> {
        Ok([0; 32])
    }
}
