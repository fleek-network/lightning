use std::net::{Ipv4Addr, SocketAddr, TcpListener, UdpSocket};

#[derive(Debug, Copy, Clone)]
pub enum Transport {
    Tcp,
    Udp,
}

pub struct PortAssigner {
    from: u16,
    to: u16,
    used_tcp: Vec<TcpListener>,
    used_udp: Vec<UdpSocket>,
}

impl PortAssigner {
    pub fn new(from: u16, to: u16) -> Self {
        Self {
            from,
            to,
            used_tcp: Vec::new(),
            used_udp: Vec::new(),
        }
    }

    pub fn next_port(&mut self, transport: Transport) -> Option<u16> {
        loop {
            let port = self.from;
            if port >= self.to {
                return None;
            }

            let addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), port);
            self.from += 1;

            match transport {
                Transport::Tcp => {
                    if let Ok(listener) = TcpListener::bind(addr) {
                        self.used_tcp.push(listener);
                        return Some(port);
                    }
                },
                Transport::Udp => {
                    if let Ok(socket) = UdpSocket::bind(addr) {
                        self.used_udp.push(socket);
                        return Some(port);
                    }
                },
            }
        }
    }
}
