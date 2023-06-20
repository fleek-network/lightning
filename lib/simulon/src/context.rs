use std::time::Duration;

use crate::scene::ServerId;

pub struct Context {}

impl Context {
    pub fn peer_list(&self) -> &Vec<ServerId> {
        todo!()
    }

    pub async fn dial(&self, _peer: &ServerId, _port: u8) -> Connection {
        todo!()
    }

    pub async fn accept(&self, _port: u8) -> Connection {
        todo!()
    }

    pub async fn sleep(&self, _duration: Duration) {
        todo!()
    }
}

pub struct Connection();

impl Connection {
    pub fn split(&mut self) -> (&mut Reader, &mut Writer) {
        todo!()
    }

    pub fn is_open(&self) -> bool {
        todo!()
    }

    pub fn close(self) {
        todo!()
    }
}

pub struct Reader {}

impl Reader {
    pub async fn recv(&mut self) {
        todo!()
    }
}

pub struct Writer {}

impl Writer {
    pub fn write(&mut self, _message: ()) {
        todo!()
    }
}
