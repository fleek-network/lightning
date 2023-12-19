use std::io::{stdout, Write};
use std::time::{Duration, SystemTime};

use lightning_interfaces::types::{EpochInfo, NodeIndex, NodeInfo};
use lightning_utils::shutdown::ShutdownController;
use tokio::pin;

use crate::rpc;

pub async fn wait_to_next_epoch(
    epoch_info: EpochInfo,
    genesis_committee: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
) {
    let shutdown_controller = ShutdownController::default();
    shutdown_controller.install_handlers();
    let shutdown_future = shutdown_controller.wait_for_shutdown();
    pin!(shutdown_future);

    tokio::select! {
        _ = &mut shutdown_future => {
            println!("Exiting...");
            std::process::exit(0);
        }
        _ = wait_loop(epoch_info, genesis_committee, rpc_client) => {
        }
    }
}

async fn wait_loop(
    epoch_info: EpochInfo,
    genesis_committee: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
) {
    let mut stdout = stdout();
    println!();
    loop {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        if now > epoch_info.epoch_end {
            let new_epoch_info =
                rpc::get_epoch_info(genesis_committee.to_vec(), rpc_client.clone())
                    .await
                    .expect("Cannot reach bootstrap nodes");
            if new_epoch_info.epoch > epoch_info.epoch {
                // The new epoch started, time to start the node.
                println!();
                println!("Start checkpointing...");
                return;
            }
            tokio::time::sleep(Duration::from_millis(2000)).await;
        } else {
            let delta = (epoch_info.epoch_end).saturating_sub(now);
            let delta = Duration::from_millis(delta);

            print!(
                "\rWaiting for new epoch to start. Joining the network in {}... (hit ENTER to exit)",
                get_timer(delta)
            );
            stdout.flush().unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

fn get_timer(duration: Duration) -> String {
    let s = duration.as_secs() % 60;
    let m = (duration.as_secs() / 60) % 60;
    let h = (duration.as_secs() / 60) / 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}
