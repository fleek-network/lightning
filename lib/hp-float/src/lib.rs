use std::fmt;

use num_bigint::BigUint;
use num_traits::{Num, Zero};

pub mod signed;
pub mod unsigned;

#[derive(Debug)]
pub enum HpFloatConversionError {
    PrecisionLevelNotSupported,
    Overflow,
    Underflow,
    DivisionError,
    FloatParseError,
}

fn format_hp_float<T, const P: usize>(value: &T, f: &mut fmt::Formatter<'_>) -> fmt::Result
where
    T: fmt::Display + fmt::Debug + Zero + PartialEq,
{
    if *value == T::zero() {
        write!(f, "0")
    } else {
        let value_str = value.to_string();
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
        write!(f, "HpUfloat<{P}>({formatted})")
    }
}

fn get_float_parts<const P: usize>(s: &str) -> (BigUint, BigUint) {
    let parts: Vec<&str> = s.split('.').collect();

    // It is safe to unwrap here since we are converting a valid f64 to a string. If the input
    // value was not a valid f64, this function wouldn't have been called in the first place.
    let integer_part = BigUint::from_str_radix(parts[0], 10).unwrap();

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
    (integer_part, fraction_part)
}
