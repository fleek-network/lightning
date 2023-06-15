use num_bigint::BigUint;
use num_traits::{Num, ToPrimitive, Zero};

#[derive(Clone, Debug)]
pub struct BigDecimal {
    pub value: BigUint,
    pub precision: u32,
}

impl Default for BigDecimal {
    fn default() -> Self {
        Self {
            value: Zero::zero(),
            precision: 18,
        }
    }
}

impl BigDecimal {
    pub fn new(value: BigUint, precision: u32) -> BigDecimal {
        BigDecimal {
            value: value * BigUint::from(10u64).pow(precision),
            precision,
        }
    }

    pub fn add(&self, other: &BigDecimal) -> BigDecimal {
        assert_eq!(self.precision, other.precision, "Precision mismatch");

        BigDecimal {
            value: &self.value + &other.value,
            precision: self.precision,
        }
    }

    pub fn sub(&self, other: &BigDecimal) -> BigDecimal {
        assert_eq!(self.precision, other.precision, "Precision mismatch");

        BigDecimal {
            value: &self.value - &other.value,
            precision: self.precision,
        }
    }

    pub fn mul(&self, other: &BigDecimal) -> BigDecimal {
        assert_eq!(self.precision, other.precision, "Precision mismatch");

        BigDecimal {
            value: &self.value * &other.value / BigUint::from(10u32).pow(self.precision),
            precision: self.precision,
        }
    }

    pub fn div(&self, other: &BigDecimal) -> BigDecimal {
        assert_eq!(self.precision, other.precision, "Precision mismatch");

        BigDecimal {
            value: &self.value * BigUint::from(10u32).pow(self.precision) / &other.value,
            precision: self.precision,
        }
    }

    pub fn to_f64(&self) -> f64 {
        let divisor = BigUint::from(10u64).pow(self.precision);
        self.value.to_f64().unwrap() / divisor.to_f64().unwrap()
    }

    pub fn from_f64(value: f64, precision: u32) -> BigDecimal {
        let s = format!("{value}");
        let parts: Vec<&str> = s.split('.').collect();

        let integer_part: BigUint = BigUint::from_str_radix(parts[0], 10).unwrap();

        let fraction_part: BigUint = if parts.len() > 1 {
            let mut frac_str = parts[1].to_string();
            while frac_str.len() < precision as usize {
                frac_str.push('0');
            }
            frac_str.truncate(precision as usize);
            BigUint::from_str_radix(&frac_str, 10).unwrap()
        } else {
            BigUint::zero()
        };

        BigDecimal {
            value: integer_part * BigUint::from(10u32).pow(precision) + fraction_part,
            precision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_big_decimal_add() {
        let decimal1 = BigDecimal::new(BigUint::from(1_000_000_000_000_000_000u64), 18);
        let decimal2 = BigDecimal::new(BigUint::from(2_000_000_000_000_000_000u64), 18);
        let sum = decimal1.add(&decimal2);
        assert_eq!(
            sum.value,
            BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128)
        );
    }

    #[test]
    fn test_big_decimal_sub() {
        let decimal1 = BigDecimal::new(BigUint::from(5_000_000_000_000_000_000u64), 18);
        let decimal2 = BigDecimal::new(BigUint::from(2_000_000_000_000_000_000u64), 18);
        let sum = decimal1.sub(&decimal2);
        assert_eq!(
            sum.value,
            BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128)
        );
    }

    #[test]
    fn test_big_decimal_mul() {
        let decimal1 = BigDecimal::new(BigUint::from(5_000_000u64), 18);
        let decimal2 = BigDecimal::new(BigUint::from(2_000_000u64), 18);
        let result = decimal1.mul(&decimal2);
        assert_eq!(
            result.value,
            BigUint::from(10_000_000_000_000_000_000_000_000_000_000u128)
        );
    }

    #[test]
    fn test_big_decimal_div() {
        let decimal1 = BigDecimal::new(BigUint::from(1u64), 18);
        let decimal2 = BigDecimal::new(BigUint::from(50u64), 18);
        let result = decimal1.div(&decimal2);
        assert_eq!(result.value, BigUint::from(20_000_000_000_000_000u128));
    }

    #[test]
    fn test_big_decimal_from_f64() {
        let decimal: f64 = 1234.567891234567;
        let result = BigDecimal::from_f64(decimal, 18);
        assert_eq!(
            result.value,
            BigUint::from(1_234_567_891_234_567_000_000u128)
        );
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
