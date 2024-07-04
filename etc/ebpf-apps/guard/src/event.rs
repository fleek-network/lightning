use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail};
use lightning_ebpf_common::{
    EventMessage,
    ACCESS_DENIED_EVENT,
    EVENT_HEADER_SIZE,
    FILE_OPEN_PROG_ID,
    LEARNING_MODE_EVENT,
    MAX_BUFFER_LEN,
    TASK_FIX_SETUID_PROG_ID,
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
        log::info!("Creating: {:?}", EVENT_LOG_FILE_PATH.as_path());
        let mut file = File::options()
            .create(true)
            .append(true)
            .open(EVENT_LOG_FILE_PATH.as_path())
            .await?;

        for message in events {
            let event_ty = match message[0] {
                ACCESS_DENIED_EVENT => "access-denied",
                LEARNING_MODE_EVENT => "learning",
                event_ty => {
                    bail!("invalid event type: {event_ty}");
                },
            };
            let prog_ty = match message[1] {
                FILE_OPEN_PROG_ID => "file-open",
                TASK_FIX_SETUID_PROG_ID => "task-fix-setuid",
                event_ty => {
                    bail!("invalid program type: {event_ty}");
                },
            };

            let nul_pos = message[EVENT_HEADER_SIZE..EVENT_HEADER_SIZE + MAX_BUFFER_LEN]
                .iter()
                .position(|c| *c == 0)
                .ok_or(anyhow!("C string is not NUL terminated"))?;
            let binary = String::from_utf8_lossy(
                message[EVENT_HEADER_SIZE..EVENT_HEADER_SIZE + nul_pos].as_ref(),
            );

            let target = match message[1] {
                FILE_OPEN_PROG_ID => {
                    let nul_pos = message[EVENT_HEADER_SIZE + MAX_BUFFER_LEN..]
                        .iter()
                        .position(|c| *c == 0)
                        .ok_or(anyhow!("C string is not NUL terminated"))?;
                    String::from_utf8_lossy(
                        message[EVENT_HEADER_SIZE + MAX_BUFFER_LEN
                            ..EVENT_HEADER_SIZE + MAX_BUFFER_LEN + nul_pos]
                            .as_ref(),
                    )
                },
                TASK_FIX_SETUID_PROG_ID => {
                    let uid_bytes: &[u8] = message[EVENT_HEADER_SIZE + MAX_BUFFER_LEN
                        ..EVENT_HEADER_SIZE + MAX_BUFFER_LEN + 4]
                        .as_ref();
                    let uid = u32::from_le_bytes(uid_bytes.try_into()?);
                    let gid_bytes: &[u8] = message[EVENT_HEADER_SIZE + MAX_BUFFER_LEN
                        ..EVENT_HEADER_SIZE + MAX_BUFFER_LEN + 4]
                        .as_ref();
                    let gid = u32::from_le_bytes(gid_bytes.try_into()?);

                    format!("uid={uid},gid={gid}").into()
                },
                _ => unreachable!(),
            };

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
