use std::{
    convert::TryInto,
    fmt,
    ops::{Add, AddAssign, Div, Mul, Sub, SubAssign},
};

use num_bigint::{
    BigInt, BigUint,
    Sign::{Minus, Plus},
};
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};
use serde::{Deserialize, Serialize};

use crate::{format_hp_float, get_float_parts, HpFloatConversionError};

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Default)]

/// A high-precision floating-point number backed by a `BigInt`.
///
/// `HpFloat` is parameterized over the precision `P`, which determines
/// the number of digits maintained after the decimal point.
///
/// # Examples
///
/// ```
/// use hp_float::HpFloat;
///
/// let x = HpFloat::<5>::from(10.12345);
/// let y = HpFloat::<5>::from(20.12345);
///
/// assert_eq!(x + y, HpFloat::<5>::from(30.24690));
/// ```
///
/// # Notes
///
/// The underlying storage is a `BigInt` from the `num-bigint` crate.
/// The precision `P` is defined at compile-time and applies to all
/// operations. When the result of an operation has more than `P`
/// digits after the decimal point, it is truncated at `P` digits.
///
/// # Type Parameters
///
/// * `P`: The number of digits to maintain after the decimal point in this `HpFloat`. Must be a
///   constant that is known at compile time.
///
/// # Attributes
///
/// * `BigInt`: The underlying large signed integer value that the `HpFloat` wraps around.
pub struct HpFloat<const P: usize>(BigInt);

impl<const P: usize> HpFloat<P> {
    pub fn new(value: BigInt) -> Self {
        let ten: BigInt = BigUint::from(10u32).into();
        HpFloat::<P>(value * ten.pow(P.try_into().unwrap()))
    }

    pub fn zero() -> HpFloat<P> {
        HpFloat::new(BigInt::zero())
    }

    pub fn convert_precision<const Q: usize>(&self) -> HpFloat<Q> {
        let current_value: &BigInt = &self.0;
        let precision_diff: i32 = P as i32 - Q as i32;
        let ten: BigInt = BigUint::from(10u32).into();

        let scaled_value: BigInt = if precision_diff > 0 {
            current_value / ten.pow(precision_diff as u32)
        } else {
            current_value * ten.pow((-precision_diff) as u32)
        };

        HpFloat::<Q>(scaled_value)
    }

    pub fn min<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 <= rhs.0 { self } else { rhs }
    }

    pub fn max<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 >= rhs.0 { self } else { rhs }
    }
}

impl<const P: usize> fmt::Display for HpFloat<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_hp_float::<BigInt, P>(&self.0, f)
    }
}

impl<const P: usize> Add<HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn add(self, rhs: HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn add(self, rhs: HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn add(self, rhs: &HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn add(self, rhs: &HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Sub<HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn sub(self, rhs: HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn sub(self, rhs: HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn sub(self, rhs: &HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn sub(self, rhs: &HpFloat<P>) -> Self::Output {
        HpFloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Mul<HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn mul(self, rhs: HpFloat<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFloat::<P>(product / scale_factor)
    }
}
impl<const P: usize> Mul<HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn mul(self, rhs: HpFloat<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFloat::<P>(product / scale_factor)
    }
}
impl<const P: usize> Mul<&HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn mul(self, rhs: &HpFloat<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFloat::<P>(product / scale_factor)
    }
}
impl<const P: usize> Mul<&HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn mul(self, rhs: &HpFloat<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFloat::<P>(product / scale_factor)
    }
}

impl<const P: usize> Div<HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn div(self, rhs: HpFloat<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFloat::<P>(dividend / &rhs.0)
    }
}
impl<const P: usize> Div<HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn div(self, rhs: HpFloat<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFloat::<P>(dividend / &rhs.0)
    }
}
impl<const P: usize> Div<&HpFloat<P>> for HpFloat<P> {
    type Output = HpFloat<P>;

    fn div(self, rhs: &HpFloat<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFloat::<P>(dividend / &rhs.0)
    }
}
impl<const P: usize> Div<&HpFloat<P>> for &HpFloat<P> {
    type Output = HpFloat<P>;

    fn div(self, rhs: &HpFloat<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFloat::<P>(dividend / &rhs.0)
    }
}

impl<const P: usize> AddAssign for HpFloat<P> {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl<const P: usize> SubAssign for HpFloat<P> {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl<const P: usize> From<BigInt> for HpFloat<P> {
    fn from(value: BigInt) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFloat(value * scale)
    }
}

impl<const P: usize> From<i16> for HpFloat<P> {
    fn from(value: i16) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i16(value).unwrap();
        HpFloat(value_to_big * scale)
    }
}
impl<const P: usize> From<i32> for HpFloat<P> {
    fn from(value: i32) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i32(value).unwrap();
        HpFloat(value_to_big * scale)
    }
}

impl<const P: usize> From<i64> for HpFloat<P> {
    fn from(value: i64) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i64(value).unwrap();
        HpFloat(value_to_big * scale)
    }
}

impl<const P: usize> From<i128> for HpFloat<P> {
    fn from(value: i128) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i128(value).unwrap();
        HpFloat(value_to_big * scale)
    }
}

impl<const P: usize> From<isize> for HpFloat<P> {
    fn from(value: isize) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_isize(value).unwrap();
        HpFloat(value_to_big * scale)
    }
}

impl<const P: usize> From<f64> for HpFloat<P> {
    fn from(value: f64) -> Self {
        let scale = BigUint::from(10u32).pow(P.try_into().unwrap());
        let mut s = format!("{value}");
        let sign = if s.starts_with('-') {
            let tail = &s[1..];
            if !tail.starts_with('+') {
                s = tail.to_owned()
            }
            Minus
        } else {
            Plus
        };

        let (integer_part, fraction_part) = get_float_parts::<P>(&s);
        let result = (integer_part * scale) + fraction_part;
        HpFloat(BigInt::from_biguint(sign, result))
    }
}

impl<const P: usize> TryFrom<HpFloat<P>> for i32 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpFloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        if interim > i32::MAX.into() {
            return Err(HpFloatConversionError::Overflow);
        } else if interim < i32::MIN.into() {
            return Err(HpFloatConversionError::Underflow);
        }
        Ok(interim.to_i32().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFloat<P>> for i64 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpFloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        if interim > i64::MAX.into() {
            return Err(HpFloatConversionError::Overflow);
        } else if interim < i64::MIN.into() {
            return Err(HpFloatConversionError::Underflow);
        }
        Ok(interim.to_i64().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFloat<P>> for i128 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpFloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        if interim > i128::MAX.into() {
            return Err(HpFloatConversionError::Overflow);
        } else if interim < i128::MIN.into() {
            return Err(HpFloatConversionError::Underflow);
        }
        Ok(interim.to_i128().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFloat<P>> for isize {
    type Error = HpFloatConversionError;

    fn try_from(value: HpFloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;

        if interim > isize::MAX.into() {
            return Err(HpFloatConversionError::Overflow);
        } else if interim < isize::MIN.into() {
            return Err(HpFloatConversionError::Underflow);
        }
        Ok(interim.to_isize().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFloat<P>> for BigInt {
    type Error = HpFloatConversionError;

    fn try_from(value: HpFloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)
    }
}

impl<const P: usize> TryFrom<HpFloat<P>> for f64 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpFloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let fraction_part = value.0.abs() % divisor.clone();
        let integer_part = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        let s = format!("{integer_part}.{fraction_part}");

        // WARNING: Truncation occurs when converting from HpFloat to f64 if the string
        // representation is longer than 18 digits. This is expected behavior due to the
        // limited precision of f64. Exercise caution and consider the potential loss of
        // precision for longer decimal values.
        s.parse::<f64>()
            .map_err(|_| HpFloatConversionError::FloatParseError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_into() {
        let large = HpFloat::<20>::new(BigInt::from(std::i64::MIN as i128 - 1));
        let medium = HpFloat::<19>::new(BigInt::from(std::i32::MAX as i64 + 1));
        let small = HpFloat::<18>::new(BigInt::from(std::i16::MAX as i32 + 1));

        assert_eq!(std::i64::MIN as i128 - 1, large.clone().try_into().unwrap());
        assert!(matches!(
            TryInto::<isize>::try_into(large.clone()),
            Err(HpFloatConversionError::Underflow)
        ));
        assert!(matches!(
            TryInto::<i64>::try_into(large.clone()),
            Err(HpFloatConversionError::Underflow)
        ));
        assert!(matches!(
            TryInto::<i32>::try_into(large.clone()),
            Err(HpFloatConversionError::Underflow)
        ));

        assert_eq!(
            TryInto::<i128>::try_into(medium.clone()).unwrap(),
            std::i32::MAX as i128 + 1
        );
        assert_eq!(
            TryInto::<i64>::try_into(medium.clone()).unwrap(),
            std::i32::MAX as i64 + 1
        );
        assert_eq!(
            TryInto::<isize>::try_into(medium.clone()).unwrap(),
            std::i32::MAX as isize + 1
        );
        assert!(matches!(
            TryInto::<i32>::try_into(medium),
            Err(HpFloatConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<i128>::try_into(small.clone()).unwrap(),
            std::i16::MAX as i128 + 1
        );
        assert_eq!(
            TryInto::<isize>::try_into(small.clone()).unwrap(),
            std::i16::MAX as isize + 1
        );
        assert_eq!(
            TryInto::<i64>::try_into(small.clone()).unwrap(),
            std::i16::MAX as i64 + 1
        );
        assert_eq!(
            TryInto::<i32>::try_into(small.clone()).unwrap(),
            std::i16::MAX as i32 + 1
        );

        let small_by_2 = &small / &200_i64.try_into().unwrap();
        let small_float: f64 = small_by_2.try_into().unwrap();
        // small_float = 32768(small) / 200   = 163.84
        assert_eq!(163.84, small_float);

        let large_by_2 = &large / &200_000_000i64.try_into().unwrap();
        // large_by_2 is HpFloat<20>(-4_611_686_018_427_387_904_500_000_000_000)
        // warn: truncation happens
        let large_float: f64 = large_by_2.try_into().unwrap();
        assert_eq!(-46116860184.27388, large_float)
    }

    #[test]
    fn test_hp_float_add() {
        let decimal1: HpFloat<18> = 1_000_000_000_000_000_000_i64.into();
        let decimal2: HpFloat<18> = 2_000_000_000_000_000_000_i64.into();
        let res: BigInt =
            BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000_u128).into();

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
    fn test_hp_float_sub() {
        let decimal1: HpFloat<18> = (-9_000_000_000_000_000_000i64).into();
        let decimal2: HpFloat<18> = (6_000_000_000_000_000_000_i64).into();
        let res: BigInt = BigInt::from(-15_000_000_000_000_000_000_000_000_000_000_000_000i128);

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
    fn test_hp_float_mul() {
        let decimal1: HpFloat<18> = 5_000_000.into();
        let decimal2: HpFloat<18> = 2_000_000.into();
        let res = BigUint::from(10_000_000_000_000_000_000_000_000_000_000u128).into();

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
    fn test_hp_float_div() {
        let decimal1: HpFloat<18> = 1.into();
        let decimal2: HpFloat<18> = 50.into();
        let res = BigUint::from(20_000_000_000_000_000u128).into();

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
    fn test_hp_float_from_f64() {
        let decimal: f64 = -1234.567891234567;
        let result = HpFloat::<18>::from(decimal);
        assert_eq!(result.0, BigInt::from(-1_234_567_891_234_567_000_000i128));
    }
    #[test]
    fn test_hp_float_from_f64_truncation() {
        #[allow(clippy::excessive_precision)]
        let decimal: f64 = 1234.5678912345678909;
        let result = HpFloat::<18>::from(decimal);
        assert_eq!(result.0, BigInt::from(1_234_567_891_234_568_000_000u128));
    }

    #[test]
    fn test_convert_precsion_up() {
        let decimal: f64 = -1234.123456;
        let decimal1 = HpFloat::<6>::from(decimal);
        let result = decimal1.convert_precision::<18>();
        assert_eq!(result.0, BigInt::from(-1_234_123_456_000_000_000_000_i128));
    }

    #[test]
    fn test_convert_precsion_down() {
        let decimal: f64 = -1234.123456;
        let decimal1 = HpFloat::<6>::from(decimal);
        let result = decimal1.convert_precision::<2>();
        assert_eq!(result.0, BigInt::from(-123_412_i128));
    }
}
