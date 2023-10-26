use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum RejectReason {
    Other,
    TooManyRequests,
    ContentNotFound,
}
