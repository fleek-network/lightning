//! The multi-threading abstractions used by the blake3's official crate.

/// The trait that abstracts over single-threaded and multi-threaded recursion.
///
/// See the [`join` module docs](index.html) for more details.
pub trait Join {
    fn join<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where
        A: FnOnce() -> RA + Send,
        B: FnOnce() -> RB + Send,
        RA: Send,
        RB: Send;
}

/// The trivial, serial implementation of `Join`. The left and right sides are
/// executed one after the other, on the calling thread. The standalone hashing
/// functions and the `Hasher::update` method use this implementation
/// internally.
///
/// See the [`join` module docs](index.html) for more details.
pub enum SerialJoin {}

impl Join for SerialJoin {
    #[inline]
    fn join<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where
        A: FnOnce() -> RA + Send,
        B: FnOnce() -> RB + Send,
        RA: Send,
        RB: Send,
    {
        (oper_a(), oper_b())
    }
}

/// The Rayon-based implementation of `Join`. The left and right sides are
/// executed on the Rayon thread pool, potentially in parallel. This
/// implementation is gated by the `rayon` feature, which is off by default.
///
/// See the [`join` module docs](index.html) for more details.
pub enum RayonJoin {}

impl Join for RayonJoin {
    #[inline]
    fn join<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where
        A: FnOnce() -> RA + Send,
        B: FnOnce() -> RB + Send,
        RA: Send,
        RB: Send,
    {
        rayon_core::join(oper_a, oper_b)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_serial_join() {
        let oper_a = || 1 + 1;
        let oper_b = || 2 + 2;
        assert_eq!((2, 4), SerialJoin::join(oper_a, oper_b));
    }

    #[test]
    fn test_rayon_join() {
        let oper_a = || 1 + 1;
        let oper_b = || 2 + 2;
        assert_eq!((2, 4), RayonJoin::join(oper_a, oper_b));
    }
}
