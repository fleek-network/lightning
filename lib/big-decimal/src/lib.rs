use std::{
    fmt,
    ops::{Add, AddAssign, Div, Mul, Sub, SubAssign},
};

use num_bigint::BigUint;
use num_traits::{zero, CheckedDiv, FromPrimitive, Num, ToPrimitive, Zero};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum BigDecimalConversionError {
    PrecisionLevelNotSupported,
    Overflow,
    DivisionError,
    FloatParseError,
}

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
        BigDecimal::<P>(value * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }

    pub fn zero() -> BigDecimal<P> {
        BigDecimal::new(zero())
    }
    pub fn convert_precision<const Q: usize>(&self) -> BigDecimal<Q> {
        let current_value: &BigUint = &self.0;

        let precision_diff: i32 = P as i32 - Q as i32;

        let scaled_value: BigUint = if precision_diff > 0 {
            current_value / BigUint::from(10u128.pow(precision_diff as u32))
        } else {
            current_value * BigUint::from(10u128.pow((-precision_diff) as u32))
        };

        BigDecimal::<Q>(scaled_value)
    }

    pub fn min<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 <= rhs.0 { self } else { rhs }
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

impl<const P: usize> Add<BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn add(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn add(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn add(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn add(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Sub<BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn sub(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn sub(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn sub(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn sub(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Mul<BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn mul(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn mul(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<&BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn mul(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<&BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn mul(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> Div<BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn div(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
    }
}
impl<const P: usize> Div<BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn div(self, rhs: BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
    }
}
impl<const P: usize> Div<&BigDecimal<P>> for BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn div(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
    }
}
impl<const P: usize> Div<&BigDecimal<P>> for &BigDecimal<P> {
    type Output = BigDecimal<P>;

    fn div(self, rhs: &BigDecimal<P>) -> Self::Output {
        BigDecimal::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
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

impl<const P: usize> From<u16> for BigDecimal<P> {
    fn from(value: u16) -> Self {
        let value_to_big: BigUint = BigUint::from_u16(value).unwrap();
        BigDecimal(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> From<u32> for BigDecimal<P> {
    fn from(value: u32) -> Self {
        let value_to_big: BigUint = BigUint::from_u32(value).unwrap();
        BigDecimal(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
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

impl<const P: usize> TryFrom<BigDecimal<P>> for f64 {
    type Error = BigDecimalConversionError;

    fn try_from(value: BigDecimal<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| BigDecimalConversionError::PrecisionLevelNotSupported)?,
        );
        let fraction_part = value.0.clone() % divisor.clone();
        let integer_part = value
            .0
            .checked_div(&divisor)
            .ok_or(BigDecimalConversionError::DivisionError)?;
        let s = format!("{integer_part}.{fraction_part}");
        s.parse::<f64>()
            .map_err(|_| BigDecimalConversionError::FloatParseError)
    }
}

impl<const P: usize> TryFrom<BigDecimal<P>> for u32 {
    type Error = BigDecimalConversionError;

    fn try_from(value: BigDecimal<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| BigDecimalConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(BigDecimalConversionError::DivisionError)?;
        interim.to_u32().ok_or(BigDecimalConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<BigDecimal<P>> for u64 {
    type Error = BigDecimalConversionError;

    fn try_from(value: BigDecimal<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| BigDecimalConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(BigDecimalConversionError::DivisionError)?;
        interim.to_u64().ok_or(BigDecimalConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<BigDecimal<P>> for u128 {
    type Error = BigDecimalConversionError;

    fn try_from(value: BigDecimal<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| BigDecimalConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(BigDecimalConversionError::DivisionError)?;
        interim.to_u128().ok_or(BigDecimalConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<BigDecimal<P>> for usize {
    type Error = BigDecimalConversionError;

    fn try_from(value: BigDecimal<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| BigDecimalConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(BigDecimalConversionError::DivisionError)?;
        interim
            .to_usize()
            .ok_or(BigDecimalConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<BigDecimal<P>> for BigUint {
    type Error = BigDecimalConversionError;

    fn try_from(value: BigDecimal<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| BigDecimalConversionError::PrecisionLevelNotSupported)?,
        );
        value
            .0
            .checked_div(&divisor)
            .ok_or(BigDecimalConversionError::DivisionError)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_try_into() {
        let large = BigDecimal::<20>::new(BigUint::from(std::u64::MAX as u128 + 1_u128));
        let medium = BigDecimal::<19>::new(BigUint::from(std::u32::MAX as u64 + 1_u64));
        let small = BigDecimal::<18>::new(BigUint::from(std::u16::MAX as u32 + 1_u32));

        assert_eq!(
            std::u64::MAX as u128 + 1_u128,
            large.clone().try_into().unwrap()
        );
        assert!(matches!(
            TryInto::<usize>::try_into(large.clone()),
            Err(BigDecimalConversionError::Overflow)
        ));
        assert!(matches!(
            TryInto::<u64>::try_into(large.clone()),
            Err(BigDecimalConversionError::Overflow)
        ));
        assert!(matches!(
            TryInto::<u32>::try_into(large),
            Err(BigDecimalConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<u128>::try_into(medium.clone()).unwrap(),
            std::u32::MAX as u128 + 1
        );
        assert_eq!(
            TryInto::<u64>::try_into(medium.clone()).unwrap(),
            std::u32::MAX as u64 + 1
        );
        assert_eq!(
            TryInto::<usize>::try_into(medium.clone()).unwrap(),
            std::u32::MAX as usize + 1
        );
        assert!(matches!(
            TryInto::<u32>::try_into(medium),
            Err(BigDecimalConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<u128>::try_into(small.clone()).unwrap(),
            std::u16::MAX as u128 + 1
        );
        assert_eq!(
            TryInto::<usize>::try_into(small.clone()).unwrap(),
            std::u16::MAX as usize + 1
        );
        assert_eq!(
            TryInto::<u64>::try_into(small.clone()).unwrap(),
            std::u16::MAX as u64 + 1
        );
        assert_eq!(
            TryInto::<u32>::try_into(small.clone()).unwrap(),
            std::u16::MAX as u32 + 1
        );

        let small_by_2 = &small / &200_u64.try_into().unwrap();
        let small_float: f64 = small_by_2.try_into().unwrap();
        // small_float = 65536(small) / 200   = 327.68
        assert_eq!(327.68, small_float);
        // Todo: more tests to test overflow and bigger gloats
    }

    #[test]
    fn test_big_decimal_add() {
        let decimal1: BigDecimal<18> = 1_000_000_000_000_000_000u64.into();
        let decimal2: BigDecimal<18> = 2_000_000_000_000_000_000u64.into();
        let res = BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128);

        let both_ref = &decimal1 + &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() + &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 + decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 + decimal2;
        assert_eq!(both_owned.0, res);
    }

    #[test]
    fn test_big_decimal_sub() {
        let decimal1: BigDecimal<18> = 5_000_000_000_000_000_000u64.into();
        let decimal2: BigDecimal<18> = 2_000_000_000_000_000_000u64.into();
        let res = BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128);

        let both_ref = &decimal1 - &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() - &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 - decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 - decimal2;
        assert_eq!(both_owned.0, res);
    }

    #[test]
    fn test_big_decimal_mul() {
        let decimal1: BigDecimal<18> = 5_000_000u64.into();
        let decimal2: BigDecimal<18> = 2_000_000u64.into();
        let res = BigUint::from(10_000_000_000_000_000_000_000_000_000_000u128);

        let both_ref = &decimal1 * &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() * &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 * decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 * decimal2;
        assert_eq!(both_owned.0, res);
    }

    #[test]
    fn test_big_decimal_div() {
        let decimal1: BigDecimal<18> = 1u64.into();
        let decimal2: BigDecimal<18> = 50u64.into();
        let res = BigUint::from(20_000_000_000_000_000u128);

        let both_ref = &decimal1 / &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() / &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 / decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 / decimal2;
        assert_eq!(both_owned.0, res);
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

    #[test]
    fn test_convert_precsion_up() {
        let decimal: f64 = 1234.123456;
        let decimal1 = BigDecimal::<6>::from(decimal);
        let result = decimal1.convert_precision::<18>();
        assert_eq!(result.0, BigUint::from(1_234_123_456_000_000_000_000_u128));
    }

    #[test]
    fn test_convert_precsion_down() {
        let decimal: f64 = 1234.123456;
        let decimal1 = BigDecimal::<6>::from(decimal);
        let result = decimal1.convert_precision::<2>();
        assert_eq!(result.0, BigUint::from(123_412_u128));
    }
}
