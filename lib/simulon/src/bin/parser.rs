//! Parse the CSV from https://wonderproxy.com/blog/a-day-in-the-life-of-the-internet/

use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use fxhash::FxHashSet;
use simulon::latency::ping::PingStat;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("You need to pass the path to the CSV file.");
        std::process::exit(-1);
    });

    let file = std::fs::File::open(&path).unwrap_or_else(|err| {
        eprintln!("Could not open '{path}'");
        eprintln!("Error: {err}");
        std::process::exit(-1);
    });

    let mut reader = BufReader::new(file)
        .lines()
        .map(|r| r.unwrap().trim().to_owned());

    if Some("\"source\",\"destination\",\"timestamp\",\"min\",\"avg\",\"max\",\"mdev\"".into())
        != reader.next()
    {
        eprintln!("The file is not the CSV I am programmed to parse. Sorry.");
        std::process::exit(-1);
    }

    let mut assigned_id = HashMap::<u32, usize>::with_capacity(512);
    let mut data = HashMap::<(usize, usize), PingStat>::with_capacity(512 * 512);

    for line in reader {
        if line.is_empty() {
            break;
        }

        let mut cols = line.split(',').map(|col| {
            let c = col.trim();
            &c[1..c.len() - 1]
        });

        let source: u32 = cols.next().unwrap().parse().unwrap();
        let destination: u32 = cols.next().unwrap().parse().unwrap();
        let _ts = cols.next().unwrap();
        let min: f64 = cols.next().unwrap().parse().unwrap();
        let avg: f64 = cols.next().unwrap().parse().unwrap();
        let max: f64 = cols.next().unwrap().parse().unwrap();
        let mdev: f64 = cols.next().unwrap().parse().unwrap();

        let next_id = assigned_id.len();
        let source = *assigned_id.entry(source).or_insert(next_id);
        let next_id = assigned_id.len();
        let destination = *assigned_id.entry(destination).or_insert(next_id);

        let min = (min * 1000.0) as u32;
        let avg = (avg * 1000.0) as u32;
        let max = (max * 1000.0) as u32;
        let mdev = (mdev * 1000.0) as u32;

        if min == 0 {
            continue;
        }

        let stat = PingStat {
            min,
            avg,
            max,
            stddev: mdev,
            count: 30,
        };

        match data.entry((source, destination)) {
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(stat);
            },
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let s = *e.get();
                *e.get_mut() = s + stat;
            },
        }
    }

    let mut eliminated = FxHashSet::default();

    let count = assigned_id.len();
    for u in 0..count {
        for v in (u + 1)..count {
            let uv = data.get(&(u, v));
            let vu = data.get(&(v, u));

            if uv.is_some() && vu.is_some() {
                continue;
            }

            if uv.is_none() && vu.is_none() {
                eliminated.insert(v);
                eliminated.insert(u);
                continue;
            }

            if let Some(x) = uv {
                data.insert((v, u), *x);
            } else {
                data.insert((u, v), *vu.unwrap());
            }
        }
    }

    println!("Unique nodes = {}", assigned_id.len());
    println!("Map size = {}", data.len());
    println!("Eliminated size = {}", eliminated.len());

    const SAME_NODE_DATA: PingStat = PingStat {
        min: 3_000,
        avg: 3_700,
        max: 5_000,
        stddev: 577,
        count: 0,
    };

    let mut buffer = Buffer::new(assigned_id.len() - eliminated.len());
    for u in 0..count {
        if eliminated.contains(&u) {
            continue;
        }

        for v in 0..count {
            if eliminated.contains(&v) {
                continue;
            }

            if u == v {
                buffer.write(&SAME_NODE_DATA);
            } else {
                let data = data.get(&(u, v)).unwrap();
                assert!(data.min > 0);
                buffer.write(data);
            }
        }
    }

    std::fs::write("./ping.bin", &buffer.0).expect("Could not write.");
}

struct Buffer(Vec<u8>);

impl Buffer {
    pub fn new(n: usize) -> Self {
        Self(Vec::with_capacity(n * n * 4 * 4))
    }

    pub fn write(&mut self, data: &PingStat) {
        let min: [u8; 4] = data.min.to_le_bytes();
        let avg: [u8; 4] = data.avg.to_le_bytes();
        let max: [u8; 4] = data.max.to_le_bytes();
        let stddev: [u8; 4] = data.stddev.to_le_bytes();
        // let count: [u8; 4] = data.count.to_le_bytes();
        self.0.extend_from_slice(&min as &[u8]);
        self.0.extend_from_slice(&avg as &[u8]);
        self.0.extend_from_slice(&max as &[u8]);
        self.0.extend_from_slice(&stddev as &[u8]);
        // self.0.extend_from_slice(&count as &[u8]);
    }
}
