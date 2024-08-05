use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};
use std::str::FromStr;

use primitive_types::U256 as EthersU256;
use ruint::aliases::U256;
use schemars::{schema_for_value, JsonSchema};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{get_float_parts, HpFixedConversionError};

/// A high-precision unsigned fixed-point number backed by a `BigUint`.
///
/// `HpUfixed` is parameterized over the precision `P`, which determines the number of digits
/// maintained after the decimal point. This structure is primarily used for accurate accounting
/// in relation to specific tokens where precision requirements are critical.
///
/// The precision `P` is defined at compile time and is crucial for ensuring accurate
/// interoperability with accounting and balances.
///
/// # Examples
///
/// ```
/// use hp_fixed::unsigned::HpUfixed;
///
/// let value: HpUfixed<18> = 123_u64.into();
/// ```
///
/// In the above example, `HpUfixed<18>` ensures a precision of 18 decimal places.
///
/// # Notes
///
/// The underlying storage is a `BigUint` from the `num-bigint` crate. When the result of an
/// operation has more than `P` digits after the decimal point, it is truncated at `P` digits.
///
/// # Type Parameters
///
/// * `P`: The number of digits to maintain after the decimal point in this `HpUfixed`. Must be a
///   constant that is known at compile time.
///
/// # Attributes
///
/// * `BigUint`: The underlying large unsigned integer value that the `HpUfixed` wraps around.

#[derive(Clone, Hash, PartialEq, PartialOrd, Ord, Eq, Default)]
pub struct HpUfixed<const P: usize>(U256);

impl<const P: usize> JsonSchema for HpUfixed<P> {
    fn schema_name() -> String {
        "HpUfixed".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::HpUfixed"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let ui = U256::from(13_u32);

        schema_for_value!(Self(ui)).schema.into()
    }
}

impl<const P: usize> HpUfixed<P> {
    pub fn new(value: U256) -> Self {
        HpUfixed::<P>(value)
    }

    pub fn zero() -> HpUfixed<P> {
        HpUfixed::new(U256::ZERO)
    }

    pub fn convert_precision<const Q: usize>(&self) -> HpUfixed<Q> {
        let current_value = self.0;

        let precision_diff: i32 = P as i32 - Q as i32;

        let scaled_value = if precision_diff > 0 {
            current_value / U256::from(10u128.pow(precision_diff as u32))
        } else {
            current_value * U256::from(10u128.pow((-precision_diff) as u32))
        };

        HpUfixed::<Q>(scaled_value)
    }

    pub fn min<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 <= rhs.0 { self } else { rhs }
    }

    pub fn max<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 >= rhs.0 { self } else { rhs }
    }

    pub fn get_value(&self) -> &U256 {
        &self.0
    }

    pub fn floor(&self) -> HpUfixed<P> {
        let scale = U256::from(10u32).pow(P.try_into().unwrap());
        let num = self.0 / scale;
        HpUfixed::<P>(num * scale)
    }
}

impl<const P: usize> fmt::Display for HpUfixed<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<const P: usize> fmt::Debug for HpUfixed<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<const P: usize> FromStr for HpUfixed<P> {
    type Err = HpFixedConversionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = U256::from_str(s).map_err(|_| HpFixedConversionError::ParseError)?;

        Ok(HpUfixed::new(value))
    }
}

impl<const P: usize> Serialize for HpUfixed<P> {
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

impl<'de, const P: usize> Deserialize<'de> for HpUfixed<P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<HpUfixed<P>>()
            .map_err(|_| serde::de::Error::custom("Failed to deserialize HpUfixed"))
    }
}

impl<const P: usize> Add<HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn add(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 + rhs.0)
    }
}

impl<const P: usize> Add<HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn add(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 + rhs.0)
    }
}

impl<const P: usize> Add<&HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn add(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 + rhs.0)
    }
}

impl<const P: usize> Add<&HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn add(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 + rhs.0)
    }
}

impl<const P: usize> Sub<HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn sub(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 - rhs.0)
    }
}

impl<const P: usize> Sub<HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn sub(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 - rhs.0)
    }
}

impl<const P: usize> Sub<&HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn sub(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 - rhs.0)
    }
}

impl<const P: usize> Sub<&HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn sub(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>(self.0 - rhs.0)
    }
}

impl<const P: usize> Mul<HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn mul(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * rhs.0) / U256::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn mul(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * rhs.0) / U256::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<&HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn mul(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * rhs.0) / U256::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<&HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn mul(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * rhs.0) / U256::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> Div<HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn div(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * U256::from(10u32).pow(P.try_into().unwrap())) / rhs.0)
    }
}
impl<const P: usize> Div<HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn div(self, rhs: HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * U256::from(10u32).pow(P.try_into().unwrap())) / rhs.0)
    }
}
impl<const P: usize> Div<&HpUfixed<P>> for HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn div(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * U256::from(10u32).pow(P.try_into().unwrap())) / rhs.0)
    }
}
impl<const P: usize> Div<&HpUfixed<P>> for &HpUfixed<P> {
    type Output = HpUfixed<P>;

    fn div(self, rhs: &HpUfixed<P>) -> Self::Output {
        HpUfixed::<P>((self.0 * U256::from(10u32).pow(P.try_into().unwrap())) / rhs.0)
    }
}

impl<const P: usize> AddAssign for HpUfixed<P> {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl<const P: usize> SubAssign for HpUfixed<P> {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl<const P: usize> From<f64> for HpUfixed<P> {
    fn from(value: f64) -> Self {
        let s = format!("{value}");

        let (integer_part, fraction_part) = get_float_parts::<P>(&s);
        HpUfixed(integer_part * U256::from(10u32).pow(P.try_into().unwrap()) + fraction_part)
    }
}

impl<const P: usize> From<u16> for HpUfixed<P> {
    fn from(value: u16) -> Self {
        let value_to_big: U256 = U256::from(value);
        HpUfixed(value_to_big * U256::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> From<u32> for HpUfixed<P> {
    fn from(value: u32) -> Self {
        let value_to_big: U256 = U256::from(value);
        HpUfixed(value_to_big * U256::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<u64> for HpUfixed<P> {
    fn from(value: u64) -> Self {
        let value_to_big: U256 = U256::from(value);
        HpUfixed(value_to_big * U256::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<u128> for HpUfixed<P> {
    fn from(value: u128) -> Self {
        let value_to_big: U256 = U256::from(value);
        HpUfixed(value_to_big * U256::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<usize> for HpUfixed<P> {
    fn from(value: usize) -> Self {
        let value_to_big: U256 = U256::from(value);
        HpUfixed(value_to_big * U256::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> TryFrom<HpUfixed<P>> for f64 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpUfixed<P>) -> Result<Self, Self::Error> {
        let divisor = U256::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let fraction_part = value.0 % divisor;
        let integer_part = value
            .0
            .checked_div(divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        let s = format!("{integer_part}.{fraction_part}");

        // WARNING: Truncation occurs when converting from HpUfixed to f64 if the string
        // representation is longer than 18 digits. This is expected behavior due to the
        // limited precision of f64. Exercise caution and consider the potential loss of
        // precision for longer decimal values.
        s.parse::<f64>()
            .map_err(|_| HpFixedConversionError::FloatParseError)
    }
}

impl<const P: usize> TryFrom<HpUfixed<P>> for u32 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpUfixed<P>) -> Result<Self, Self::Error> {
        let divisor = U256::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;

        interim
            .try_into()
            .map_err(|_| HpFixedConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<HpUfixed<P>> for u64 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpUfixed<P>) -> Result<Self, Self::Error> {
        let divisor = U256::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        interim
            .try_into()
            .map_err(|_| HpFixedConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<HpUfixed<P>> for u128 {
    type Error = HpFixedConversionError;

    fn try_from(value: HpUfixed<P>) -> Result<Self, Self::Error> {
        let divisor = U256::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        interim
            .try_into()
            .map_err(|_| HpFixedConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<HpUfixed<P>> for usize {
    type Error = HpFixedConversionError;

    fn try_from(value: HpUfixed<P>) -> Result<Self, Self::Error> {
        let divisor = U256::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFixedConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(divisor)
            .ok_or(HpFixedConversionError::DivisionError)?;
        interim
            .try_into()
            .map_err(|_| HpFixedConversionError::Overflow)
    }
}

#[cfg(feature = "ruint")]
impl<const P: usize> From<HpUfixed<P>> for U256 {
    fn from(value: HpUfixed<P>) -> Self {
        value.0
    }
}

impl<const P: usize> From<HpUfixed<P>> for EthersU256 {
    fn from(value: HpUfixed<P>) -> Self {
        EthersU256::from(value.0)
    }
}

impl<const P: usize> From<EthersU256> for HpUfixed<P> {
    fn from(value: EthersU256) -> Self {
        Self::new(value.into())
    }
}

#[cfg(test)]
mod tests {

    use ruint::aliases::U256;

    use super::*;

    #[test]
    fn test_floor() {
        let num = HpUfixed::<6>::from(1.3);
        assert_eq!(num.floor(), HpUfixed::<6>::from(1.0));
        let num = HpUfixed::<6>::from(12.9);
        assert_eq!(num.floor(), HpUfixed::<6>::from(12.0));
        let num = HpUfixed::<6>::from(1223.91323);
        assert_eq!(num.floor(), HpUfixed::<6>::from(1223.0));
    }

    #[test]
    fn test_try_into() {
        let large = HpUfixed::<20>::from(u64::MAX as u128 + 1_u128);
        let medium = HpUfixed::<19>::from(u32::MAX as u64 + 1_u64);
        let small = HpUfixed::<18>::from(u16::MAX as u32 + 1_u32);

        assert_eq!(u64::MAX as u128 + 1_u128, large.clone().try_into().unwrap());
        assert!(matches!(
            TryInto::<usize>::try_into(large.clone()),
            Err(HpFixedConversionError::Overflow)
        ));
        assert!(matches!(
            TryInto::<u64>::try_into(large.clone()),
            Err(HpFixedConversionError::Overflow)
        ));
        assert!(matches!(
            TryInto::<u32>::try_into(large),
            Err(HpFixedConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<u128>::try_into(medium.clone()).unwrap(),
            u32::MAX as u128 + 1
        );
        assert_eq!(
            TryInto::<u64>::try_into(medium.clone()).unwrap(),
            u32::MAX as u64 + 1
        );
        assert_eq!(
            TryInto::<usize>::try_into(medium.clone()).unwrap(),
            u32::MAX as usize + 1
        );
        assert!(matches!(
            TryInto::<u32>::try_into(medium),
            Err(HpFixedConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<u128>::try_into(small.clone()).unwrap(),
            u16::MAX as u128 + 1
        );
        assert_eq!(
            TryInto::<usize>::try_into(small.clone()).unwrap(),
            u16::MAX as usize + 1
        );
        assert_eq!(
            TryInto::<u64>::try_into(small.clone()).unwrap(),
            u16::MAX as u64 + 1
        );
        assert_eq!(
            TryInto::<u32>::try_into(small.clone()).unwrap(),
            u16::MAX as u32 + 1
        );

        let small_by_2 = &small / &200_u64.into();
        let small_float: f64 = small_by_2.try_into().unwrap();
        // small_float = 65536(small) / 200   = 327.68
        assert_eq!(327.68, small_float);
        // Todo: more tests to test overflow and bigger gloats
    }

    #[test]
    fn test_hp_fixed_add() {
        let decimal1: HpUfixed<18> = 1_000_000_000_000_000_000u64.into();
        let decimal2: HpUfixed<18> = 2_000_000_000_000_000_000u64.into();
        let res = U256::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128);

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
        let decimal1: HpUfixed<18> = 5_000_000_000_000_000_000u64.into();
        let decimal2: HpUfixed<18> = 2_000_000_000_000_000_000u64.into();
        let res = U256::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128);

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
        let decimal1: HpUfixed<18> = 5_000_000u64.into();
        let decimal2: HpUfixed<18> = 2_000_000u64.into();
        let res = U256::from(10_000_000_000_000_000_000_000_000_000_000u128);

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
        let decimal1: HpUfixed<18> = 1u64.into();
        let decimal2: HpUfixed<18> = 50u64.into();
        let res = U256::from(20_000_000_000_000_000u128);

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
        let decimal: f64 = 1234.567891234567;
        let result = HpUfixed::<18>::from(decimal);
        assert_eq!(result.0, U256::from(1_234_567_891_234_567_000_000u128));
    }

    #[test]
    fn test_hp_fixed_from_f64_truncation() {
        #[allow(clippy::excessive_precision)]
        let decimal: f64 = 1234.5678912345678909;
        let result = HpUfixed::<18>::from(decimal);
        assert_eq!(result.0, U256::from(1_234_567_891_234_568_000_000u128));
    }

    #[test]
    fn test_convert_precsion_up() {
        let decimal: f64 = 1_234.123456;
        let decimal1 = HpUfixed::<6>::from(decimal);
        let result = decimal1.convert_precision::<18>();
        assert_eq!(result.0, U256::from(1_234_123_456_000_000_000_000_u128));
    }

    #[test]
    fn test_convert_precsion_down() {
        let decimal: f64 = 1234.123456;
        let decimal1 = HpUfixed::<6>::from(decimal);
        let result = decimal1.convert_precision::<2>();
        assert_eq!(result.0, U256::from(123_412_u128));
    }

    #[test]
    fn test_serde() {
        let decimal: HpUfixed<18> = HpUfixed::from(10_f64);
        let ser = serde_json::to_string(&decimal).unwrap();
        let decimal2: HpUfixed<18> = serde_json::from_str(&ser).unwrap();
        assert_eq!(decimal, decimal2);
    }

    #[test]
    fn bincode_serde_test() {
        let decimal: HpUfixed<18> = HpUfixed::from(10_f64);
        let serialized = bincode::serialize(&decimal).expect("Failed to serialize using bincode");
        let deserialized: HpUfixed<18> =
            bincode::deserialize(&serialized).expect("Failed to deserialize using bincode");
        assert_eq!(decimal, deserialized);
    }

    #[test]
    fn test_ufixed_from_ethers_u256() {
        let uint = primitive_types::U256::from(329399399321_u64);
        let fixed: HpUfixed<18> = HpUfixed::from(uint);
        let target = U256::from(329399399321_u64);
        assert_eq!(fixed.0, target);

        let uint = primitive_types::U256::from(0_u64);
        let fixed: HpUfixed<18> = HpUfixed::from(uint);
        let target = U256::from(0_u64);
        assert_eq!(fixed.0, target);

        let uint = primitive_types::U256::from(938563839392332232455_u128);
        let fixed: HpUfixed<18> = HpUfixed::from(uint);
        let target = U256::from(938563839392332232455_u128);
        assert_eq!(fixed.0, target);
    }

    #[test]
    fn test_ethers_256_from_ufixed() {
        let fixed: HpUfixed<0> = HpUfixed::from(329399399321_u64);
        let target = primitive_types::U256::from(329399399321_u64);
        assert_eq!(target, fixed.into());

        let fixed: HpUfixed<0> = HpUfixed::from(0_u64);
        let target = primitive_types::U256::from(0_u64);
        assert_eq!(target, fixed.into());

        let fixed: HpUfixed<0> = HpUfixed::from(938563839392332232455_u128);
        let target = primitive_types::U256::from(938563839392332232455_u128);
        assert_eq!(target, fixed.into());
    }
}
