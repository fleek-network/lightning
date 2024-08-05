use std::convert::TryInto;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};
use std::str::FromStr;

use num_bigint::Sign::{self, Minus, Plus};
use num_bigint::{BigInt, BigUint};
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};
use schemars::{schema_for_value, JsonSchema};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{format_hp_fixed, get_float_parts_b, HpFixedConversionError};

#[derive(Clone, Hash, PartialEq, PartialOrd, Ord, Eq, Default)]

/// A high-precision fixed-point number backed by a `BigInt`.
///
/// `HpFixed` is parameterized over the precision `P`, which determines
/// the number of digits maintained after the decimal point.
///
/// # Examples
///
/// ```
/// use hp_fixed::signed::HpFixed;
///
/// let x = HpFixed::<5>::from(10.12345);
/// let y = HpFixed::<5>::from(20.12345);
///
/// assert_eq!(x + y, HpFixed::<5>::from(30.24690));
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
/// * `P`: The number of digits to maintain after the decimal point in this `HpFixed`. Must be a
///   constant that is known at compile time.
///
/// # Attributes
///
/// * `BigInt`: The underlying large signed integer value that the `HpFixed` wraps around.
pub struct HpFixed<const P: usize>(BigInt);

impl JsonSchema for HpFixed<6> {
    fn schema_name() -> String {
        "HpUfixed".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::HpFixed"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let ui = BigInt::new(Sign::Plus, vec![1, 1]);

        schema_for_value!(Self(ui)).schema.into()
    }
}

impl JsonSchema for HpFixed<18> {
    fn schema_name() -> String {
        "HpUfixed".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::HpFixed"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let ui = BigInt::new(Sign::Plus, vec![1, 1]);

        schema_for_value!(Self(ui)).schema.into()
    }
}

impl<const P: usize> HpFixed<P> {
    pub fn new(value: BigInt) -> Self {
        HpFixed::<P>(value)
    }

    pub fn zero() -> HpFixed<P> {
        HpFixed::new(BigInt::zero())
    }

    pub fn convert_precision<const Q: usize>(&self) -> HpFixed<Q> {
        let current_value: &BigInt = &self.0;
        let precision_diff: i32 = P as i32 - Q as i32;
        let ten: BigInt = BigUint::from(10u32).into();

        let scaled_value: BigInt = if precision_diff > 0 {
            current_value / ten.pow(precision_diff as u32)
        } else {
            current_value * ten.pow((-precision_diff) as u32)
        };

        HpFixed::<Q>(scaled_value)
    }

    pub fn min<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 <= rhs.0 { self } else { rhs }
    }

    pub fn max<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 >= rhs.0 { self } else { rhs }
    }

    pub fn try_abs(&self) -> Option<HpFixed<P>> {
        let big_int = BigInt::try_from(self.clone()).ok()?;
        Some(Self::from(big_int.abs()))
    }

    pub fn floor(&self) -> HpFixed<P> {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let num = &self.0 / &scale;
        HpFixed::<P>(num * scale)
    }
}

impl<const P: usize> fmt::Display for HpFixed<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_hp_fixed::<BigInt, P>(&self.0, f, false)
    }
}

impl<const P: usize> fmt::Debug for HpFixed<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_hp_fixed::<BigInt, P>(&self.0, f, true)
    }
}

impl<const P: usize> FromStr for HpFixed<P> {
    type Err = HpFixedConversionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = BigInt::from_str(s).map_err(|_| HpFixedConversionError::ParseError)?;

        Ok(HpFixed::new(value))
    }
}

impl<const P: usize> Serialize for HpFixed<P> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let s = &self.to_string();
        let cleaned_s = s.replace('_', "");
        let parts: Vec<&str> = cleaned_s.split('<').collect();
        let final_string = parts[0].to_string();

        serializer.serialize_str(&final_string)
    }
}

impl<'de, const P: usize> Deserialize<'de> for HpFixed<P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<HpFixed<P>>()
            .map_err(|_| serde::de::Error::custom("Failed to deserialize HpFixed"))
    }
}

impl<const P: usize> Add<HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn add(self, rhs: HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn add(self, rhs: HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn add(self, rhs: &HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn add(self, rhs: &HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Sub<HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn sub(self, rhs: HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn sub(self, rhs: HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn sub(self, rhs: &HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn sub(self, rhs: &HpFixed<P>) -> Self::Output {
        HpFixed::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Mul<HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn mul(self, rhs: HpFixed<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFixed::<P>(product / scale_factor)
    }
}
impl<const P: usize> Mul<HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn mul(self, rhs: HpFixed<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFixed::<P>(product / scale_factor)
    }
}
impl<const P: usize> Mul<&HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn mul(self, rhs: &HpFixed<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFixed::<P>(product / scale_factor)
    }
}
impl<const P: usize> Mul<&HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn mul(self, rhs: &HpFixed<P>) -> Self::Output {
        let product = &self.0 * &rhs.0;
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFixed::<P>(product / scale_factor)
    }
}

impl<const P: usize> Div<HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn div(self, rhs: HpFixed<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFixed::<P>(dividend / &rhs.0)
    }
}
impl<const P: usize> Div<HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn div(self, rhs: HpFixed<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFixed::<P>(dividend / &rhs.0)
    }
}
impl<const P: usize> Div<&HpFixed<P>> for HpFixed<P> {
    type Output = HpFixed<P>;

    fn div(self, rhs: &HpFixed<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFixed::<P>(dividend / &rhs.0)
    }
}
impl<const P: usize> Div<&HpFixed<P>> for &HpFixed<P> {
    type Output = HpFixed<P>;

    fn div(self, rhs: &HpFixed<P>) -> Self::Output {
        let scale_factor: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let dividend = &self.0 * scale_factor;
        HpFixed::<P>(dividend / &rhs.0)
    }
}

impl<const P: usize> AddAssign for HpFixed<P> {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl<const P: usize> SubAssign for HpFixed<P> {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl<const P: usize> From<BigInt> for HpFixed<P> {
    fn from(value: BigInt) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        HpFixed(value * scale)
    }
}

impl<const P: usize> From<i16> for HpFixed<P> {
    fn from(value: i16) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i16(value).unwrap();
        HpFixed(value_to_big * scale)
    }
}
impl<const P: usize> From<i32> for HpFixed<P> {
    fn from(value: i32) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i32(value).unwrap();
        HpFixed(value_to_big * scale)
    }
}

impl<const P: usize> From<i64> for HpFixed<P> {
    fn from(value: i64) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i64(value).unwrap();
        HpFixed(value_to_big * scale)
    }
}

impl<const P: usize> From<i128> for HpFixed<P> {
    fn from(value: i128) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_i128(value).unwrap();
        HpFixed(value_to_big * scale)
    }
}

impl<const P: usize> From<isize> for HpFixed<P> {
    fn from(value: isize) -> Self {
        let scale: BigInt = BigUint::from(10u32).pow(P.try_into().unwrap()).into();
        let value_to_big: BigInt = BigInt::from_isize(value).unwrap();
        HpFixed(value_to_big * scale)
    }
}

impl<const P: usize> From<f64> for HpFixed<P> {
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

        let (integer_part, fraction_part) = get_float_parts_b::<P>(&s);
        let result = (integer_part * scale) + fraction_part;
        HpFixed(BigInt::from_biguint(sign, result))
    }
}

impl<const P: usize> TryFrom<HpFixed<P>> for i32 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpFixed<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        if interim > i32::MAX.into() {
            return Err(HpFixedConversionError::Overflow);
        } else if interim < i32::MIN.into() {
            return Err(HpFixedConversionError::Underflow);
        }
        Ok(interim.to_i32().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFixed<P>> for i64 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpFixed<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        if interim > i64::MAX.into() {
            return Err(HpFixedConversionError::Overflow);
        } else if interim < i64::MIN.into() {
            return Err(HpFixedConversionError::Underflow);
        }
        Ok(interim.to_i64().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFixed<P>> for i128 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpFixed<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        if interim > i128::MAX.into() {
            return Err(HpFixedConversionError::Overflow);
        } else if interim < i128::MIN.into() {
            return Err(HpFixedConversionError::Underflow);
        }
        Ok(interim.to_i128().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFixed<P>> for isize {
    type Error = HpFixedConversionError;

    fn try_from(value: HpFixed<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;

        if interim > isize::MAX.into() {
            return Err(HpFixedConversionError::Overflow);
        } else if interim < isize::MIN.into() {
            return Err(HpFixedConversionError::Underflow);
        }
        Ok(interim.to_isize().unwrap())
    }
}

impl<const P: usize> TryFrom<HpFixed<P>> for BigInt {
    type Error = HpFixedConversionError;

    fn try_from(value: HpFixed<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        value
            .0
            .checked_div(&divisor)
            .ok_or(HpFixedConversionError::DivisionError)
    }
}

impl<const P: usize> TryFrom<HpFixed<P>> for f64 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpFixed<P>) -> Result<Self, Self::Error> {
        let divisor = BigInt::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let fraction_part = value.0.abs() % divisor.clone();
        let integer_part = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        let s = format!("{integer_part}.{fraction_part}");

        // WARNING: Truncation occurs when converting from HpFixed to f64 if the string
        // representation is longer than 18 digits. This is expected behavior due to the
        // limited precision of f64. Exercise caution and consider the potential loss of
        // precision for longer decimal values.
        s.parse::<f64>()
            .map_err(|_| HpFixedConversionError::FloatParseError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floor() {
        let num = HpFixed::<6>::from(-1.3);
        assert_eq!(num.floor(), HpFixed::<6>::from(-1.0));
        let num = HpFixed::<6>::from(12.9);
        assert_eq!(num.floor(), HpFixed::<6>::from(12.0));
        let num = HpFixed::<6>::from(1223.91323);
        assert_eq!(num.floor(), HpFixed::<6>::from(1223.0));
    }

    #[test]
    fn test_try_into() {
        let large = HpFixed::<20>::from(BigInt::from(i64::MIN as i128 - 1));
        let medium = HpFixed::<19>::from(BigInt::from(i32::MAX as i64 + 1));
        let small = HpFixed::<18>::from(BigInt::from(i16::MAX as i32 + 1));

        assert_eq!(i64::MIN as i128 - 1, large.clone().try_into().unwrap());
        assert!(matches!(
            TryInto::<isize>::try_into(large.clone()),
            Err(HpFixedConversionError::Underflow)
        ));
        assert!(matches!(
            TryInto::<i64>::try_into(large.clone()),
            Err(HpFixedConversionError::Underflow)
        ));
        assert!(matches!(
            TryInto::<i32>::try_into(large.clone()),
            Err(HpFixedConversionError::Underflow)
        ));

        assert_eq!(
            TryInto::<i128>::try_into(medium.clone()).unwrap(),
            i32::MAX as i128 + 1
        );
        assert_eq!(
            TryInto::<i64>::try_into(medium.clone()).unwrap(),
            i32::MAX as i64 + 1
        );
        assert_eq!(
            TryInto::<isize>::try_into(medium.clone()).unwrap(),
            i32::MAX as isize + 1
        );
        assert!(matches!(
            TryInto::<i32>::try_into(medium),
            Err(HpFixedConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<i128>::try_into(small.clone()).unwrap(),
            i16::MAX as i128 + 1
        );
        assert_eq!(
            TryInto::<isize>::try_into(small.clone()).unwrap(),
            i16::MAX as isize + 1_isize
        );
        assert_eq!(
            TryInto::<i64>::try_into(small.clone()).unwrap(),
            i16::MAX as i64 + 1
        );
        assert_eq!(
            TryInto::<i32>::try_into(small.clone()).unwrap(),
            i16::MAX as i32 + 1
        );

        let small_by_2 = &small / &200_i64.into();
        let small_float: f64 = small_by_2.try_into().unwrap();
        // small_float = 32768(small) / 200   = 163.84
        assert_eq!(163.84, small_float);

        let large_by_2 = &large / &200_000_000i64.into();
        // large_by_2 is HpFixed<20>(-4_611_686_018_427_387_904_500_000_000_000)
        // warn: truncation happens
        let large_float: f64 = large_by_2.try_into().unwrap();
        assert_eq!(-46116860184.27388, large_float)
    }

    #[test]
    fn test_hp_fixed_add() {
        let decimal1: HpFixed<18> = 1_000_000_000_000_000_000_i64.into();
        let decimal2: HpFixed<18> = 2_000_000_000_000_000_000_i64.into();
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
    fn test_hp_fixed_sub() {
        let decimal1: HpFixed<18> = (-9_000_000_000_000_000_000i64).into();
        let decimal2: HpFixed<18> = (6_000_000_000_000_000_000_i64).into();
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
    fn test_hp_fixed_mul() {
        let decimal1: HpFixed<18> = 5_000_000.into();
        let decimal2: HpFixed<18> = 2_000_000.into();
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
    fn test_hp_fixed_div() {
        let decimal1: HpFixed<18> = 1.into();
        let decimal2: HpFixed<18> = 50.into();
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
    fn test_hp_fixed_from_f64() {
        let decimal: f64 = -1234.567891234567;
        let result = HpFixed::<18>::from(decimal);
        assert_eq!(result.0, BigInt::from(-1_234_567_891_234_567_000_000i128));
    }
    #[test]
    fn test_hp_fixed_from_f64_truncation() {
        #[allow(clippy::excessive_precision)]
        let decimal: f64 = 1234.5678912345678909;
        let result = HpFixed::<18>::from(decimal);
        assert_eq!(result.0, BigInt::from(1_234_567_891_234_568_000_000u128));
    }

    #[test]
    fn test_convert_precsion_up() {
        let decimal: f64 = -1234.123456;
        let decimal1 = HpFixed::<6>::from(decimal);
        let result = decimal1.convert_precision::<18>();
        assert_eq!(result.0, BigInt::from(-1_234_123_456_000_000_000_000_i128));
    }

    #[test]
    fn test_convert_precsion_down() {
        let decimal: f64 = -1234.123456;
        let decimal1 = HpFixed::<6>::from(decimal);
        let result = decimal1.convert_precision::<2>();
        assert_eq!(result.0, BigInt::from(-123_412_i128));
    }

    #[test]
    fn test_try_abs() {
        let num = HpFixed::<18>::from(-10);
        let target = HpFixed::<18>::from(10);
        assert_eq!(num.try_abs().unwrap(), target);
    }
}
