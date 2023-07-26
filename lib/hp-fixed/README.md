# hp-fixed

The hp-fixed library provides high-precision signed (HpFixed) and unsigned (HpUfixed) fixed-point decimal backed by the num-bigint crate, aiming for accurate computations when precision requirements are critical.

These types are parameterized over the precision P, which determines the number of digits maintained after the decimal point. Precision P is defined at compile-time and applies to all operations.

## Usage

```toml
[dependencies]
hp_fixed = "0.1"
```

### HpFixed(signed)

HpFixed is a high precision signed fixed-point number backed by a BigInt.

Here is a quick example showcasing how to utilize HpFixed:
```rust
use hp_fixed::signed::HpFixed;

async fn main() {
    let x = HpFixed::<5>::from(10.12345);
    let y = HpFixed::<5>::from(20.12345);
    let z = x + y;

    assert_eq!(z, HpFixed::<5>::from(30.24690)); 

    let value = HpFixed::<19>::new(BigInt::from(std::i32::MAX as i64 + 1));
    assert_eq!(
    TryInto::<isize>::try_into(value.clone()).unwrap(),
    std::i32::MAX as isize + 1
);   
}
```

### HpUfixed(unsigned)

HpUfixed is a high precision signed fixed-point number backed by a BigUint.

Here is a quick example showcasing how to utilize HpUfixed:
```rust
async fn main() {
    use hp_fixed::unsigned::HpUfixed;
    use num_bigint::BigUint;
    
    let a = HpUfixed::<5>::new(BigUint::from(10u32));
    let b = HpUfixed::<5>::new(BigUint::from(20u32));
    let c = a + b;
    
    assert_eq!(c, HpUfixed::<5>::new(BigUint::from(30u32)));
    
    let value = HpUfixed::<20>::new(BigUint::from(std::u64::MAX as u128 + 1_u128));
    assert_eq!(
        std::u64::MAX as u128 + 1_u128,
        value.clone().try_into().unwrap()
    );
}
```


## Contributing
Contributions to hp_fixed are welcomed. Please make sure to run the test suite before opening a pull request

## License
[MIT](https://github.com/fleek-network/draco/blob/main/lib/hp-fixed/LICENSE-MIT)
[APACHE 2.0](https://github.com/fleek-network/draco/blob/main/lib/hp-fixed/LICENSE-APACHE)


