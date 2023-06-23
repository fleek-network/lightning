use std::{future::Future, time::Duration};

use crate::{connection::Connection, peer::PeerId};

pub struct Context {}

impl Context {
    pub fn peer_list(&self) -> &Vec<PeerId> {
        todo!()
    }

    pub async fn dial(&self, _peer: &PeerId, _port: u8) -> Option<Connection> {
        todo!()
    }

    pub async fn accept(&self, _port: u8) -> Option<Connection> {
        todo!()
    }

    pub async fn sleep(&self, _duration: Duration) {
        todo!()
    }

    pub async fn yield_execution() {
        todo!()
    }

    pub fn spawn(&self, _future: impl Future) {
        todo!()
    }
}
