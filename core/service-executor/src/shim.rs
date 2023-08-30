pub struct Service {
    pub start: fn(OnStartArgs),
    pub connected: fn(OnConnectedArgs),
    pub disconnected: fn(OnDisconnectedArgs),
    pub message: fn(OnMessageArgs),
    pub respond: fn(OnEventResponseArgs),
}

pub struct ServiceExecutor<C: Collection> {
    collection: PhantomData<C>,
}
