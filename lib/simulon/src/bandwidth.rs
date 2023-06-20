use std::{
    fmt::Debug,
    ops::{Add, Div, Mul},
    time::Duration,
};

/// The bandwidth of a server. Stored as bit/ms.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Bandwidth(usize);

impl Bandwidth {
    /// Convert from a Kbit/s.
    #[inline]
    pub fn kbps(rate: usize) -> Self {
        Bandwidth(rate)
    }

    /// Convert from a Mbit/s.
    #[inline]
    pub fn mbps(rate: usize) -> Self {
        Self::kbps(rate * 1000)
    }

    /// Convert from a Gbit/s.
    #[inline]
    pub fn gbps(rate: usize) -> Self {
        Self::mbps(rate * 1000)
    }

    /// Convert from a Tbit/s.
    #[inline]
    pub fn tbps(rate: usize) -> Self {
        Self::gbps(rate * 1000)
    }

    /// Returns the bandwidth in bit per ms.
    #[inline]
    pub fn as_bps(&self) -> usize {
        self.0
    }

    /// Returns if the bandwidth is zero.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl Debug for Bandwidth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bit_per_sec = Duration::from_secs(1) * self;

        const UNITS: &[&str] = &["Tb", "Gb", "Mb", "Kb", "bit"];
        let mut i = 0;
        let mut unit = 1_000_000_000_000;

        while unit > 0 {
            if bit_per_sec >= unit {
                let n = bit_per_sec / unit;
                let u = UNITS[i];
                return write!(f, "{n}{u}/s");
            }

            i += 1;
            unit /= 1000;
        }

        write!(f, "0b/s")
    }
}

impl Add<Bandwidth> for Bandwidth {
    type Output = Self;

    fn add(self, rhs: Bandwidth) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<&Bandwidth> for Bandwidth {
    type Output = Bandwidth;

    fn add(self, rhs: &Bandwidth) -> Self::Output {
        Bandwidth(self.0 + rhs.0)
    }
}

impl Add<Bandwidth> for &Bandwidth {
    type Output = Bandwidth;

    fn add(self, rhs: Bandwidth) -> Self::Output {
        Bandwidth(self.0 + rhs.0)
    }
}

impl Add<&Bandwidth> for &Bandwidth {
    type Output = Bandwidth;

    fn add(self, rhs: &Bandwidth) -> Self::Output {
        Bandwidth(self.0 + rhs.0)
    }
}

impl Mul<usize> for Bandwidth {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Mul<usize> for &Bandwidth {
    type Output = Bandwidth;

    fn mul(self, rhs: usize) -> Self::Output {
        Bandwidth(self.0 * rhs)
    }
}

impl Mul<Bandwidth> for usize {
    type Output = Bandwidth;

    fn mul(self, rhs: Bandwidth) -> Self::Output {
        Bandwidth(rhs.0 * self)
    }
}

impl Mul<&Bandwidth> for usize {
    type Output = Bandwidth;

    fn mul(self, rhs: &Bandwidth) -> Self::Output {
        Bandwidth(rhs.0 * self)
    }
}

impl Div<usize> for Bandwidth {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl Div<usize> for &Bandwidth {
    type Output = Bandwidth;

    fn div(self, rhs: usize) -> Self::Output {
        Bandwidth(self.0 / rhs)
    }
}

impl Mul<Duration> for Bandwidth {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: Duration) -> Self::Output {
        rhs.as_millis() * (self.0 as u128)
    }
}

impl Mul<Duration> for &Bandwidth {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: Duration) -> Self::Output {
        rhs.as_millis() * (self.0 as u128)
    }
}

impl Mul<&Duration> for Bandwidth {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: &Duration) -> Self::Output {
        rhs.as_millis() * (self.0 as u128)
    }
}

impl Mul<&Duration> for &Bandwidth {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: &Duration) -> Self::Output {
        rhs.as_millis() * (self.0 as u128)
    }
}

impl Mul<Bandwidth> for Duration {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: Bandwidth) -> Self::Output {
        self.as_millis() * (rhs.0 as u128)
    }
}

impl Mul<&Bandwidth> for Duration {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: &Bandwidth) -> Self::Output {
        self.as_millis() * (rhs.0 as u128)
    }
}

impl Mul<Bandwidth> for &Duration {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: Bandwidth) -> Self::Output {
        self.as_millis() * (rhs.0 as u128)
    }
}

impl Mul<&Bandwidth> for &Duration {
    type Output = u128;

    /// Returns the number of bits that can be transferred during this time.
    fn mul(self, rhs: &Bandwidth) -> Self::Output {
        self.as_millis() * (rhs.0 as u128)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(Bandwidth::kbps(1000), Bandwidth::mbps(1));
        assert_eq!(Bandwidth::kbps(1000 * 1000), Bandwidth::gbps(1));
        assert_eq!(Bandwidth::kbps(1000 * 1000 * 1000), Bandwidth::tbps(1));
        assert_eq!(Bandwidth::kbps(1000) * 5, Bandwidth::mbps(5));
        assert_eq!(5 * Bandwidth::kbps(1000), 5 * Bandwidth::mbps(1));
        assert_eq!(5 * Bandwidth::kbps(1000), Bandwidth::mbps(1) * 5);
        assert_eq!(Bandwidth::kbps(1000) * 7, 7 * Bandwidth::mbps(1));
        assert_eq!(format!("{:?}", Bandwidth::kbps(128)), "128Kb/s");
        assert_eq!(format!("{:?}", Bandwidth::mbps(17)), "17Mb/s");
        assert_eq!(format!("{:?}", Bandwidth::mbps(1)), "1Mb/s");
        assert_eq!(format!("{:?}", Bandwidth::gbps(1)), "1Gb/s");
        assert_eq!(format!("{:?}", Bandwidth::tbps(1)), "1Tb/s");
    }
}
