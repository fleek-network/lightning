#[derive(Clone, Copy, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum RejectReason {
    TooManyRequests = 1,
    ContentNotFound = 2,
    Other = 3,
}
