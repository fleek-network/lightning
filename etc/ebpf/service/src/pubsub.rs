use std::io;

use bytes::Bytes;
use resolved_pathbuf::ResolvedPathBuf;
use tokio::net::UnixStream;

use crate::frame::Message;
use crate::utils;

pub struct Subscriber {
    socket: UnixStream,
}

impl Subscriber {
    pub async fn subscribe(topic: u32) -> io::Result<Self> {
        let path = ResolvedPathBuf::try_from("~/.lightning/ebpf")?;
        let sock = UnixStream::connect(path.as_path()).await?;
        utils::write(&sock, Bytes::from(topic.to_le_bytes().to_vec())).await?;
        Ok(Self { socket: sock })
    }

    /// Return the next message.
    ///
    /// Safety: this is not cancel safe.
    pub async fn next(&self) -> io::Result<Message> {
        while let Some(_bytes) = utils::read(&self.socket).await? {}
        todo!()
    }
}
