use std::io::{stdin, stdout, Write};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime};

use lightning_interfaces::types::{EpochInfo, NodeIndex, NodeInfo};

use crate::rpc;

pub fn wait_to_next_epoch(
    epoch_info: EpochInfo,
    genesis_committee: &[(NodeIndex, NodeInfo)],
    rpc_client: &reqwest::Client,
) {
    let mut stdout = stdout();
    let shutdown_rx = spawn_stdin_listener();
    loop {
        if shutdown_rx.try_recv().is_ok() {
            println!("Exiting...");
            std::process::exit(0);
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        if now > epoch_info.epoch_end {
            let new_epoch_info = rpc::sync_call(rpc::get_epoch_info(
                genesis_committee.to_vec(),
                rpc_client.clone(),
            ))
            .expect("Cannot reach bootstrap nodes");
            if new_epoch_info.epoch > epoch_info.epoch {
                // The new epoch started, time to start the node.
                println!();
                println!("Start checkpointing...");
                return;
            }
            std::thread::sleep(Duration::from_millis(2000));
        } else {
            let delta = (epoch_info.epoch_end).saturating_sub(now);
            let delta = Duration::from_millis(delta);

            print!(
                "\rWaiting for new epoch to start. Joining the network in {}... (hit ENTER to exit)",
                get_timer(delta)
            );
            stdout.flush().unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn spawn_stdin_listener() -> Receiver<()> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let mut buffer = String::new();
            stdin().read_line(&mut buffer).unwrap();
            tx.send(()).unwrap();
        }
    });
    rx
}

fn get_timer(duration: Duration) -> String {
    let s = duration.as_secs() % 60;
    let m = (duration.as_secs() / 60) % 60;
    let h = (duration.as_secs() / 60) / 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}
