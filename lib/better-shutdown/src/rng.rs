// The code here is borrowed from tokio
//
// https://github.com/tokio-rs/tokio/blob/d33fdd86a3de75500fe554d6547cf5ad43e006bf
//  /tokio/src/util/rand.rs

use std::cell::UnsafeCell;

use rand::rngs::OsRng;
use rand::Rng;

thread_local!(
    static THREAD_RNG_KEY: UnsafeCell<FastRand> = UnsafeCell::new(FastRand::new());
);

pub fn fast_rng(n: u32) -> u32 {
    THREAD_RNG_KEY.with(|r| { unsafe { &mut *r.get() } }.fastrand_n(n))
}

/// Fast random number generate.
///
/// Implement `xorshift64+`: 2 32-bit `xorshift` sequences added together.
/// Shift triplet `[17,7,16]` was calculated as indicated in Marsaglia's
/// `Xorshift` paper: <https://www.jstatsoft.org/article/view/v008i14/xorshift.pdf>
/// This generator passes the SmallCrush suite, part of TestU01 framework:
/// <http://simul.iro.umontreal.ca/testu01/tu01.html>
#[derive(Clone, Copy, Debug)]
struct FastRand {
    one: u32,
    two: u32,
}

impl FastRand {
    pub fn new() -> Self {
        let n = OsRng.gen::<u64>();
        FastRand {
            one: (n >> 32) as u32,
            two: n as u32,
        }
    }

    pub fn fastrand_n(&mut self, n: u32) -> u32 {
        // This is similar to fastrand() % n, but faster.
        // See https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
        let mul = (self.fastrand() as u64).wrapping_mul(n as u64);
        (mul >> 32) as u32
    }

    fn fastrand(&mut self) -> u32 {
        let mut s1 = self.one;
        let s0 = self.two;

        s1 ^= s1 << 17;
        s1 = s1 ^ s0 ^ s1 >> 7 ^ s0 >> 16;

        self.one = s0;
        self.two = s1;

        s0.wrapping_add(s1)
    }
}
