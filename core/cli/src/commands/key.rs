use anyhow::Result;

use crate::args::KeySubCmd;

pub async fn exec(key: KeySubCmd) -> Result<()> {
    match key {
        KeySubCmd::Show => show_key().await,
        KeySubCmd::Generate => generate_key().await,
    }
}

async fn generate_key() -> Result<()> {
    Ok(())
}

async fn show_key() -> Result<()> {
    Ok(())
}
