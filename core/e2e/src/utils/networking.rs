use std::{
    collections::HashSet,
    net::{TcpListener, UdpSocket},
};

#[derive(Debug, Copy, Clone)]
pub enum Transport {
    Tcp,
    Udp,
}

#[derive(Default)]
pub struct PortAssigner {
    tcp_ports: HashSet<u16>,
    udp_ports: HashSet<u16>,
}

impl PortAssigner {
    pub fn get_port(&mut self, from: u16, to: u16, transport: Transport) -> Option<u16> {
        for port in from..=to {
            match transport {
                Transport::Tcp => {
                    if !self.tcp_ports.contains(&port) && is_port_available(port, transport) {
                        self.tcp_ports.insert(port);
                        return Some(port);
                    }
                },
                Transport::Udp => {
                    if !self.udp_ports.contains(&port) && is_port_available(port, transport) {
                        self.udp_ports.insert(port);
                        return Some(port);
                    }
                },
            }
        }
        None
    }
}

pub fn get_available_port(from: u16, to: u16, transport: Transport) -> Option<u16> {
    (from..=to).find(|port| is_port_available(*port, transport))
}

fn is_port_available(port: u16, transport: Transport) -> bool {
    match transport {
        Transport::Tcp => TcpListener::bind(("127.0.0.1", port)).is_ok(),
        Transport::Udp => UdpSocket::bind(format!("127.0.0.1:{port}")).is_ok(),
    }
}
