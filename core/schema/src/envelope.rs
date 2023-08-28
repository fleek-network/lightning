use std::marker::PhantomData;
use std::sync::Arc;

use crate::LightningMessage;

/// A pre-encoded message envelope.
pub struct Envelope<T>(Arc<[u8]>, PhantomData<T>);

impl<T> AsRef<[u8]> for Envelope<T> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T> Clone for Envelope<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<T: LightningMessage> Envelope<T> {
    pub fn new(msg: T) -> std::io::Result<Self> {
        let mut buffer = Vec::with_capacity(512);
        msg.encode(&mut buffer)?;
        Ok(Self(buffer.into(), PhantomData))
    }
}
