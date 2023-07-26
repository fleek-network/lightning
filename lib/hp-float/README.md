# hp-float

The hp-float library provides high-precision signed (HpFloat) and unsigned (HpUfloat) floating-point types backed by the num-bigint crate, aiming for accurate computations when precision requirements are critical.

These types are parameterized over the precision P, which determines the number of digits maintained after the decimal point. Precision P is defined at compile-time and applies to all operations.

## Usage

```toml
[dependencies]
hp_float = "0.1"
```

## HpFloat(signed)

HpFloat is a high precision signed floating-point number backed by a BigInt.

Here is a quick example showcasing how to utilize HpFloat:
```rust
use hp_float::signed::HpFloat;

async fn main() {
    let x = HpFloat::<5>::from(10.12345);
    let y = HpFloat::<5>::from(20.12345);
    let z = x + y;

    assert_eq!(z, HpFloat::<5>::from(30.24690)); 

    let value = HpFloat::<19>::new(BigInt::from(std::i32::MAX as i64 + 1));
    assert_eq!(
    TryInto::<isize>::try_into(value.clone()).unwrap(),
    std::i32::MAX as isize + 1
);   
}
```

## HpUfloat(unsigned)

HpUfloat is a high precision signed floating-point number backed by a BigUint.

Here is a quick example showcasing how to utilize HpUfloat:
```rust
async fn main() {
    use hp_float::unsigned::HpUfloat;
    use num_bigint::BigUint;
    
    let a = HpUfloat::<5>::new(BigUint::from(10u32));
    let b = HpUfloat::<5>::new(BigUint::from(20u32));
    let c = a + b;
    
    assert_eq!(c, HpUfloat::<5>::new(BigUint::from(30u32)));
    
    let value = HpUfloat::<20>::new(BigUint::from(std::u64::MAX as u128 + 1_u128));
    assert_eq!(
        std::u64::MAX as u128 + 1_u128,
        value.clone().try_into().unwrap()
    );
}
```


## Contributing
Contributions to hp_float are welcomed. Please make sure to run the test suite before opening a pull request

## License
[MIT]
[APACHE 2.0]


