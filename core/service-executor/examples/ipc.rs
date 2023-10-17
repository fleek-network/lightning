use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::Interest;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Notify;

static TIME: Mutex<Option<Instant>> = Mutex::new(None);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let notify = Arc::new(Notify::new());

    let notifier = notify.clone();
    tokio::spawn(async move {
        let listener = UnixListener::bind("bind_path").unwrap();
        notifier.notify_one();

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("connection: {addr:?}");
                    tokio::spawn(async {
                        match run_stream_loop(stream).await {
                            Ok(_) => {},
                            Err(e) => {
                                println!("error! {e:?}");
                            },
                        }
                    });
                },
                Err(_) => { /* connection failed */ },
            }
        }
    });

    notify.notified().await;

    let stream = UnixStream::connect("bind_path").await?;

    loop {
        let mut guard = TIME.lock().unwrap();
        if guard.is_none() {
            println!("start");
            let now = std::time::Instant::now();
            *guard = Some(now);
        }
        drop(guard);

        let ready = stream.ready(Interest::WRITABLE).await?;

        if ready.is_writable() {
            // Try to write data, this may still fail with `WouldBlock`
            // if the readiness event is a false positive.
            match stream.try_write(&[0; 120]) {
                Ok(n) => {
                    println!("write {} bytes", n);
                },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(e.into());
                },
                Err(e) => {
                    return Err(e.into());
                },
            }
        }

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn run_stream_loop(stream: UnixStream) -> Result<(), Box<dyn Error>> {
    let mut data = vec![0; 120];

    loop {
        let ready = stream.ready(Interest::READABLE).await?;

        if ready.is_readable() {
            // Try to read data, this may still fail with `WouldBlock`
            // if the readiness event is a false positive.
            match stream.try_read(&mut data) {
                Ok(0) => {
                    println!("read EOF");
                    return Ok(());
                },
                Ok(n) => {
                    println!("read {} bytes", n);

                    let now = std::time::Instant::now();
                    let started = TIME.lock().unwrap().take().unwrap();
                    println!("took {:?}", now.duration_since(started));
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    println!("x");
                    continue;
                },
                Err(e) => {
                    return Err(e.into());
                },
            }
        }
    }
}
