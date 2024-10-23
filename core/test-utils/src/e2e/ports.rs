// Copyright (c) Fleek Network
// SPDX-License-Identifier: Apache-2.0, MIT

// Derived from https://github.com/MystenLabs/sui/blob/bbfaafc17652d221651e835c908028c440f039d7/narwhal/config/src/utils.rs
// Extended to also bind the UDP port.

use std::net::{TcpListener, TcpStream, UdpSocket};

/// Return an ephemeral, available port. On unix systems, the port returned will be in the
/// TIME_WAIT state ensuring that the OS won't hand out this port for some grace period.
/// Callers should be able to bind to this port given they use SO_REUSEADDR.
pub fn get_available_port(host: &str) -> u16 {
    const MAX_PORT_RETRIES: u32 = 1000;

    for _ in 0..MAX_PORT_RETRIES {
        if let Ok(port) = get_ephemeral_tcp_port(host) {
            if bind_udp_port(host, port).is_ok() {
                return port;
            }
        }
    }

    panic!("Error: could not find an available port");
}

fn get_ephemeral_tcp_port(host: &str) -> std::io::Result<u16> {
    // Request a random available port from the OS
    let listener = TcpListener::bind((host, 0))?;
    let addr = listener.local_addr()?;

    // Create and accept a connection (which we'll promptly drop) in order to force the port
    // into the TIME_WAIT state, ensuring that the port will be reserved from some limited
    // amount of time (roughly 60s on some Linux systems)
    let _sender = TcpStream::connect(addr)?;
    let _incoming = listener.accept()?;

    Ok(addr.port())
}

fn bind_udp_port(host: &str, port: u16) -> std::io::Result<()> {
    let _ = UdpSocket::bind((host, port))?;

    Ok(())
}
