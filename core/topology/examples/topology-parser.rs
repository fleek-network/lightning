use std::collections::{BTreeMap, BTreeSet};
use std::env::args;
use std::error::Error;
use std::path::Path;
use std::process::exit;

use csv::{ReaderBuilder, WriterBuilder};

#[derive(Debug, serde::Deserialize)]
struct PingData {
    source: u16,
    destination: u16,
    avg: f32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ServerData {
    id: u16,
    title: String,
    country: String,
    latitude: f32,
    longitude: f32,
}
#[allow(unused)]
#[derive(Debug, serde::Serialize)]
struct DSOutput {
    src: u16,
    dst: u16,
    latency: f32,
}

#[derive(Debug)]
struct Node {
    map: BTreeMap<u16, f32>,
    data: ServerData,
}

fn run<P: AsRef<Path>>(ping: P, servers: P) -> Result<(), Box<dyn Error>> {
    println!("reading server data");
    let mut server_info = BTreeMap::<u16, ServerData>::new();
    let mut ids = Vec::new();
    let mut rdr = ReaderBuilder::new().from_path(servers)?;
    for result in rdr.deserialize() {
        let record: ServerData = result?;
        ids.push(record.id);
        server_info.insert(record.id, record);
    }

    println!("reading ping data");
    let mut map = BTreeMap::<u16, Node>::new();
    let mut rdr = ReaderBuilder::new().from_path(ping)?;
    for result in rdr.deserialize() {
        let mut record: PingData = result?;
        let entry = map.entry(record.source).or_insert(Node {
            map: BTreeMap::new(),
            data: {
                if let Some(info) = server_info.get(&record.source) {
                    info.clone()
                } else {
                    continue;
                }
            },
        });
        if record.source == record.destination {
            record.avg = 0.;
        }
        entry.map.insert(record.destination, record.avg);
    }

    println!("mirroring and pruning missing latencies");
    let mut invalid_ids = BTreeSet::new();
    for src in &ids {
        for dst in &ids {
            let src_info = map.get(src).unwrap();

            if !src_info.map.contains_key(dst) {
                // src -> dst unavailable, check dst -> src
                let dst_info = map.get(dst).unwrap();
                if let Some(&latency) = dst_info.map.get(src) {
                    // insert to src -> dst
                    map.get_mut(src).unwrap().map.insert(*dst, latency);
                } else {
                    // we have no data between these nodes, so we ignore them
                    invalid_ids.insert(*src);
                    invalid_ids.insert(*dst);
                }
            }
        }
    }

    println!("building final dataset");
    let mut metadata = Vec::new();
    let mut set: Vec<Vec<f32>> = Vec::new();
    for src in &ids {
        if invalid_ids.contains(src) {
            continue;
        }
        let node = map.get(src).unwrap();
        let mut buf = Vec::new();
        for dst in &ids {
            if invalid_ids.contains(dst) {
                continue;
            }

            let latency = node.map.get(dst).unwrap();
            buf.push(*latency);
        }

        let data = ServerData {
            id: set.len() as u16,
            ..node.data.clone()
        };
        metadata.push(data);
        set.push(buf);
    }

    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .from_path("matrix.csv")?;
    for line in set {
        wtr.serialize(&line)?;
    }
    wtr.flush()?;
    println!("saved matrix.csv");

    let mut wtr = WriterBuilder::new().from_path("metadata.csv")?;
    for line in metadata {
        wtr.serialize(&line)?;
    }
    wtr.flush()?;
    println!("saved metadata.csv");

    Ok(())
}

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() < 3 {
        eprintln!("usage: cargo run --bin topology-parser -- <ping data> <server data>");
        exit(1);
    }
    if let Err(e) = run(&args[1], &args[2]) {
        eprintln!("error: {e:?}");
    }
}
