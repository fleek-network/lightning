use async_trait::async_trait;
use draco_interfaces::{
    Blake3Hash, CompressionAlgorithm, IncrementalPutInterface, PutFeedProofError, PutFinalizeError,
    PutWriteError,
};

pub struct IncrementalPut;

#[async_trait]
impl IncrementalPutInterface for IncrementalPut {
    fn feed_proof(&mut self, _proof: &[u8]) -> Result<(), PutFeedProofError> {
        todo!()
    }

    fn write(
        &mut self,
        _content: &[u8],
        _compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError> {
        todo!()
    }

    fn is_finished(&self) -> bool {
        todo!()
    }

    async fn finalize(self) -> Result<Blake3Hash, PutFinalizeError> {
        todo!()
    }
}
