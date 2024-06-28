use anyhow::bail;
use log::error;
use tokio::net::UnixStream;

use crate::frame::{IpcServiceFrame, Pf};
use crate::map::SharedMap;
use crate::utils;

pub struct Connection {
    socket: UnixStream,
    shared_state: SharedMap,
}

impl Connection {
    pub fn new(socket: UnixStream, shared_state: SharedMap) -> Self {
        Self {
            socket,
            shared_state,
        }
    }

    #[inline]
    async fn pf_handle(&mut self, message: Pf) -> anyhow::Result<()> {
        match message.op {
            Pf::ADD => {
                self.shared_state.packet_filter_add(message.addr).await?;
            },
            Pf::REMOVE => {
                self.shared_state.packet_filter_remove(message.addr).await?;
            },
            op => {
                bail!("invalid op: {op:?}");
            },
        }
        Ok(())
    }

    #[inline]
    async fn handle_request(&mut self, frame: IpcServiceFrame) -> anyhow::Result<()> {
        match frame {
            IpcServiceFrame::Pf(pf) => self.pf_handle(pf).await,
        }
    }

    /// Safety: This is not cancel safe.
    pub async fn handle(mut self) -> anyhow::Result<()> {
        while let Some(bytes) = utils::read(&self.socket).await? {
            match IpcServiceFrame::try_from(bytes.as_ref()) {
                Ok(f) => {
                    if let Err(e) = self.handle_request(f).await {
                        error!("failed to handle request: {e:?}");
                    }
                },
                Err(e) => {
                    error!("failed to deserialize frame: {e:?}");
                },
            }
        }

        Ok(())
    }
}
