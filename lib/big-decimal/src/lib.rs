use std::{
    fmt,
    ops::{Add, AddAssign, Div, Mul, Sub, SubAssign},
};

use num_bigint::BigUint;
use num_traits::{zero, FromPrimitive, Num, ToPrimitive, Zero};
use serde::{Deserialize, Serialize};

/// `BigDecimal` is a structure that encapsulates a `BigUint` while enforcing specific precision
/// rules.
///
/// This structure is primarily used for accounting purposes in relation to FLK and STABLE tokens.
/// The precision requirement defined at compile time with the const generic parameter `P`, is
/// critical for ensuring accurate interoperation with L2 accounting and balances.
///
/// # Type Parameters
///
/// * `P`: A const parameter of type `usize` that determines the precision of the `BigDecimal`.
///
/// # Example
///
/// ```
/// use big_decimal::BigDecimal;
/// let value: BigDecimal<18> = 123_u64.into();
/// ```
///
/// In the above example, `BigDecimal<18>` ensures a precision of 18 decimal places.
///
/// # Attributes
///
/// * `BigUint`: The underlying large unsigned integer value that the `BigDecimal` wraps around.

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Default)]
pub struct BigDecimal<const P: usize>(BigUint);

impl<const P: usize> BigDecimal<P> {
    pub fn new(value: BigUint) -> Self {
        BigDecimal::<P>(value * 10u128.pow(P.try_into().unwrap()))
    }

    pub fn zero() -> BigDecimal<P> {
        BigDecimal::new(zero())
    }
}

impl<const P: usize> fmt::Display for BigDecimal<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value_str = self.0.to_string();
        let chars: Vec<char> = value_str.chars().collect();

        let mut formatted = String::new();
        let mut count = 0;

        for i in (0..chars.len()).rev() {
            formatted.push(chars[i]);
            count += 1;

            if count % 3 == 0 && i != 0 {
                formatted.push('_');
            }
        }
        formatted = formatted.chars().rev().collect();
        write!(f, "BigDecimal<{P}>({formatted})")
    }
}

impl<const P: usize> Add for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn add(self, other: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 + &other.0)
    }
}

impl<const P: usize> Sub for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn sub(self, other: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 - &other.0)
    }
}

impl<const P: usize> Mul for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn mul(self, other: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * &other.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> Div for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn div(self, other: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &other.0)
    }
}

impl<const P: usize> AddAssign for BigDecimal<P> {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl<const P: usize> SubAssign for BigDecimal<P> {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl<const P: usize> From<f64> for BigDecimal<P> {
    fn from(value: f64) -> Self {
        let s = format!("{value}");
        let parts: Vec<&str> = s.split('.').collect();

        let integer_part: BigUint = BigUint::from_str_radix(parts[0], 10).unwrap();

        let fraction_part: BigUint = if parts.len() > 1 {
            let mut frac_str = parts[1].to_string();
            while frac_str.len() < P {
                frac_str.push('0');
            }
            frac_str.truncate(P);
            BigUint::from_str_radix(&frac_str, 10).unwrap()
        } else {
            BigUint::zero()
        };

        BigDecimal(integer_part * BigUint::from(10u32).pow(P.try_into().unwrap()) + fraction_part)
    }
}

impl<const P: usize> From<BigUint> for BigDecimal<P> {
    fn from(value: BigUint) -> Self {
        BigDecimal(value * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<u64> for BigDecimal<P> {
    fn from(value: u64) -> Self {
        let value_to_big: BigUint = BigUint::from_u64(value).unwrap();
        BigDecimal(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<u128> for BigDecimal<P> {
    fn from(value: u128) -> Self {
        let value_to_big: BigUint = BigUint::from_u128(value).unwrap();
        BigDecimal(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<usize> for BigDecimal<P> {
    fn from(value: usize) -> Self {
        let value_to_big: BigUint = BigUint::from_usize(value).unwrap();
        BigDecimal(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<BigDecimal<P>> for BigUint {
    fn from(value: BigDecimal<P>) -> Self {
        let divisor = BigUint::from(10u64).pow(P.try_into().unwrap());
        value.0 / divisor
    }
}

impl<const P: usize> From<BigDecimal<P>> for f64 {
    fn from(value: BigDecimal<P>) -> Self {
        let divisor = BigUint::from(10u64).pow(P.try_into().unwrap());
        value.0.to_f64().unwrap_or_default() / divisor.to_f64().unwrap()
    }
}

impl<const P: usize> From<BigDecimal<P>> for u64 {
    fn from(value: BigDecimal<P>) -> Self {
        let divisor = BigUint::from(10u64).pow(P.try_into().unwrap());
        value.0.to_u64().unwrap_or_default() / divisor.to_u64().unwrap()
    }
}

impl<const P: usize> From<BigDecimal<P>> for u128 {
    fn from(value: BigDecimal<P>) -> Self {
        let divisor = BigUint::from(10u64).pow(P.try_into().unwrap());
        value.0.to_u128().unwrap_or_default() / divisor.to_u128().unwrap()
    }
}

impl<const P: usize> From<BigDecimal<P>> for usize {
    fn from(value: BigDecimal<P>) -> Self {
        let divisor = BigUint::from(10u64).pow(P.try_into().unwrap());
        value.0.to_usize().unwrap_or_default() / divisor.to_usize().unwrap()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_big_decimal_add() {
        let decimal1: BigDecimal<18> = 1_000_000_000_000_000_000u64.into();
        let decimal2: BigDecimal<18> = 2_000_000_000_000_000_000u64.into();
        let sum = decimal1 + decimal2;
        assert_eq!(
            sum.0,
            BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128)
        );
    }

    #[test]
    fn test_big_decimal_sub() {
        let decimal1: BigDecimal<18> = 5_000_000_000_000_000_000u64.into();
        let decimal2: BigDecimal<18> = 2_000_000_000_000_000_000u64.into();
        let sum = decimal1 - decimal2;
        assert_eq!(
            sum.0,
            BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128)
        );
    }

    #[test]
    fn test_big_decimal_mul() {
        let decimal1: BigDecimal<18> = 5_000_000u64.into();
        let decimal2: BigDecimal<18> = 2_000_000u64.into();
        let result = decimal1 * decimal2;
        assert_eq!(
            result.0,
            BigUint::from(10_000_000_000_000_000_000_000_000_000_000u128)
        );
    }

    #[test]
    fn test_big_decimal_div() {
        let decimal1: BigDecimal<18> = 1u64.into();
        let decimal2: BigDecimal<18> = 50u64.into();
        let result = decimal1 / decimal2;
        assert_eq!(result.0, BigUint::from(20_000_000_000_000_000u128));
    }

    #[test]
    fn test_big_decimal_from_f64() {
        let decimal: f64 = 1234.567891234567;
        let result = BigDecimal::<18>::from(decimal);
        assert_eq!(result.0, BigUint::from(1_234_567_891_234_567_000_000u128));
    }
    // #[test]
    // fn test_big_decimal_from_f64_truncation() {
    //     let decimal: f64 = 1234.56789123456789;
    //     let result = BigDecimal::from_f64(decimal, 18);
    //     assert_eq!(
    //         result.value,
    //         BigUint::from(1_234_567_891_234_568_000_000u128)
    //     );
    // }
}
