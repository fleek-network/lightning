use bytes::BytesMut;

struct Io {}

pub struct Fd(FdInner);

enum FdInner {
    Tokio(),
    BytesMut { buffer: BytesMut },
}

impl Fd {
    pub async fn write(&mut self, buffer: &[u8]) -> std::io::Result<()> {
        Ok(())
    }
}
