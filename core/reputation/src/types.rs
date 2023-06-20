use std::ops::{Add, Div, Mul, Sub};

pub trait WeightedValue {
    fn get_weighted_value(&self) -> f64;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WeightedFloat {
    pub(crate) value: f64,
    pub(crate) weight: f64,
}

impl WeightedValue for WeightedFloat {
    fn get_weighted_value(&self) -> f64 {
        self.value * self.weight
    }
}

impl Default for WeightedFloat {
    fn default() -> Self {
        WeightedFloat {
            value: 0.0,
            weight: 1.0,
        }
    }
}

impl Add<WeightedFloat> for WeightedFloat {
    type Output = WeightedFloat;

    fn add(self, rhs: WeightedFloat) -> Self::Output {
        WeightedFloat {
            value: self.value + rhs.value,
            weight: self.weight,
        }
    }
}

impl Div for WeightedFloat {
    type Output = WeightedFloat;

    fn div(self, rhs: Self) -> Self::Output {
        WeightedFloat {
            value: self.value / rhs.value,
            weight: self.weight,
        }
    }
}

impl Mul for WeightedFloat {
    type Output = WeightedFloat;

    fn mul(self, rhs: Self) -> Self::Output {
        WeightedFloat {
            value: self.value * rhs.value,
            weight: self.weight,
        }
    }
}

impl Sub for WeightedFloat {
    type Output = WeightedFloat;

    fn sub(self, rhs: Self) -> Self::Output {
        WeightedFloat {
            value: self.value - rhs.value,
            weight: self.weight,
        }
    }
}

impl From<f64> for WeightedFloat {
    fn from(value: f64) -> Self {
        WeightedFloat { value, weight: 1.0 }
    }
}

impl PartialEq for WeightedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl PartialOrd for WeightedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}
