use crate::transports::Transport;

pub struct Context {
    //..
}

fn spawn_transport_loop<T: Transport>(config: T::Config) {}
