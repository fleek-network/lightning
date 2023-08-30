use fn_sdk::internal::{
    OnConnectedArgs,
    OnDisconnectedArgs,
    OnEventResponseArgs,
    OnMessageArgs,
    OnStartArgs,
};

pub struct Service {
    pub start: fn(OnStartArgs),
    pub connected: fn(OnConnectedArgs),
    pub disconnected: fn(OnDisconnectedArgs),
    pub message: fn(OnMessageArgs),
    pub respond: fn(OnEventResponseArgs),
}
