use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::bail;
use lightning_ebpf_common::{
    EventMessage,
    ACCESS_DENIED_EVENT,
    EVENT_HEADER_SIZE,
    FILE_OPEN_PROG_ID,
    MAX_BUFFER_LEN,
};
use lightning_utils::config::LIGHTNING_HOME_DIR;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub static EVENT_LOG_FILE_PATH: Lazy<PathBuf> =
    Lazy::new(|| LIGHTNING_HOME_DIR.join("ebpf/events.json"));

#[derive(Deserialize, Serialize)]
pub struct Record {
    pub ty: String,
    pub prog: String,
    pub timestamp: u128,
    pub binary: String,
    pub target: String,
}
pub async fn write_events(events: Vec<EventMessage>) -> anyhow::Result<()> {
    if !events.is_empty() {
        let mut file = File::options()
            .create(true)
            .append(true)
            .open(EVENT_LOG_FILE_PATH.as_path())
            .await?;

        for event in events {
            let event_ty = match event[0] {
                ACCESS_DENIED_EVENT => "access-denied",
                event_ty => {
                    bail!("invalid event type: {event_ty}");
                },
            };
            let prog_ty = match event[1] {
                FILE_OPEN_PROG_ID => "file-open",
                event_ty => {
                    bail!("invalid program type: {event_ty}");
                },
            };
            let binary = String::from_utf8_lossy(
                event[EVENT_HEADER_SIZE..EVENT_HEADER_SIZE + MAX_BUFFER_LEN].as_ref(),
            );
            let target =
                String::from_utf8_lossy(event[EVENT_HEADER_SIZE + MAX_BUFFER_LEN..].as_ref());

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time to go forwards")
                .as_millis();

            let mut event_json = serde_json::to_string::<Record>(&Record {
                ty: event_ty.to_string(),
                prog: prog_ty.to_string(),
                timestamp,
                binary: binary.to_string(),
                target: target.to_string(),
            })?;
            event_json.push('\n');

            file.write_all(event_json.as_bytes()).await?;
        }
    }

    Ok(())
}
