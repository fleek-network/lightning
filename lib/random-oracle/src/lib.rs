// inspired by https://gist.github.com/qti3e/ed5b1e06514957f7032d3b41ff362c34

/// `RandomOracle` is a verifiable randomness generator using a transcript of input data.
///
/// The struct includes a `domain` as a unique identifier, a `data` vector containing the input data
/// (as tuples of label and data in byte vectors), and an optional `prefix` that can be used for
/// all new labels
///
/// By using a hash function on the provided input data, the `RandomOracle` provides a way to
/// generate and verify randomness.
///
/// # Example
///
/// ```
/// use random_oracle::RandomOracle;
/// let mut oracle = RandomOracle::empty("example domain");
/// oracle = oracle.with("label", &123u64);
/// let result = oracle.compile();
/// ```
///
/// # Panics
///
/// Methods of this struct may panic if called with a duplicated label. To avoid this, ensure
/// labels are unique and do not repeat.
///
/// # Notes
///
/// The `domain` field is not just for descriptive purposes. It's used as a key in the Blake3
/// hashing function, so changing it will result in different hashes for the same input data.
pub struct RandomOracle {
    domain: String,
    data: Vec<(Vec<u8>, Vec<u8>)>,
    /// A prefix for all the new labels.
    prefix: Option<String>,
}

/// A trait defining the interface for any data that can be passed as input to the RandomOracle.
/// Implementers of this trait must provide a constant type identifier (TYPE) and a method
/// (to_random_oracle_input) to return serialized bytes for the current data.
pub trait RandomOracleInput {
    /// A unique type identifier of this data type.
    const TYPE: &'static str;

    /// Return serialized bytes for the current data.
    fn to_random_oracle_input(&self) -> Vec<u8>;
}

impl RandomOracle {
    /// Create a new random oracle.
    pub fn empty(domain: impl Into<String>) -> Self {
        RandomOracle {
            domain: domain.into(),
            data: Vec::new(),
            prefix: None,
        }
    }

    pub fn get_domain(&self) -> &str {
        &self.domain
    }

    /// Merges another `RandomOracle` instance into the current one. The function mutates the
    /// current instance, extending its data with the data from the other instance. The function
    /// consumes the other instance and returns the current instance.
    pub fn merge(mut self, other: Self) -> Self {
        for data in other.data {
            self.append_data(data)
        }
        self
    }

    /// Set a prefix for the new values passed to the random oracle, this can be used when you
    /// want to use the random oracle in a loop.
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// Provide the oracle with the given input and label, the label MUST be unique. The ordering
    /// of the data does not matter.
    pub fn with<T: RandomOracleInput>(mut self, label: &'static str, data: &T) -> Self {
        let mut key = match &self.prefix {
            Some(prefix) => format!("{prefix}/{label}").to_random_oracle_input(),
            None => label.to_random_oracle_input(),
        };
        key.extend_from_slice(&(key.len() as u64).to_be_bytes());

        let mut value = data.to_random_oracle_input();
        value.extend_from_slice(&(value.len() as u64).to_be_bytes());
        value.extend_from_slice(b"/");
        value.extend_from_slice(T::TYPE.as_bytes());
        value.extend_from_slice(b"0");

        let new_pair = (key, value);
        self.append_data(new_pair);

        self
    }

    /// Appends a tuple of vectors of u8 to the data of the RandomOracle. The function sorts the
    /// data vector to ensure that labels are unique.
    ///
    ///  # Panics
    ///
    /// If called with a duplicate label
    fn append_data(&mut self, data: (Vec<u8>, Vec<u8>)) {
        match self.data.binary_search_by(|(k, _)| k.cmp(&data.0)) {
            Ok(_) => panic!("duplicate label"),
            Err(idx) => self.data.insert(idx, data),
        };
    }

    /// Returns a combined input stream from all the inputs this oracle has already
    /// been given.
    pub fn compile(&self) -> Vec<u8> {
        let mut result: Vec<u8> = Vec::new();
        for (key, value) in &self.data {
            result.extend(key);
            result.extend(value);
        }
        result
    }
}

impl RandomOracleInput for Option<i32> {
    const TYPE: &'static str = "i32";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_random_oracle_input(),
            None => vec![],
        }
    }
}

impl RandomOracleInput for i32 {
    const TYPE: &'static str = "i32";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl RandomOracleInput for Option<i64> {
    const TYPE: &'static str = "i64";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_random_oracle_input(),
            None => vec![],
        }
    }
}

impl RandomOracleInput for i64 {
    const TYPE: &'static str = "i64";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl RandomOracleInput for u8 {
    const TYPE: &'static str = "u8";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl RandomOracleInput for Option<u8> {
    const TYPE: &'static str = "u8";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_random_oracle_input(),
            None => vec![],
        }
    }
}

impl RandomOracleInput for u16 {
    const TYPE: &'static str = "u16";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl RandomOracleInput for Option<u16> {
    const TYPE: &'static str = "u16";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_random_oracle_input(),
            None => vec![],
        }
    }
}

impl RandomOracleInput for Option<u32> {
    const TYPE: &'static str = "u32";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_random_oracle_input(),
            None => vec![],
        }
    }
}

impl RandomOracleInput for u32 {
    const TYPE: &'static str = "u32";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl RandomOracleInput for u64 {
    const TYPE: &'static str = "u64";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl RandomOracleInput for Option<u64> {
    const TYPE: &'static str = "u64";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_random_oracle_input(),
            None => vec![],
        }
    }
}

impl RandomOracleInput for u128 {
    const TYPE: &'static str = "u128";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl RandomOracleInput for Option<u128> {
    const TYPE: &'static str = "u128";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_random_oracle_input(),
            None => vec![],
        }
    }
}

impl RandomOracleInput for String {
    const TYPE: &'static str = "String";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl RandomOracleInput for Option<String> {
    const TYPE: &'static str = "String";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.as_bytes().to_vec(),
            None => Vec::new(),
        }
    }
}

impl<'a> RandomOracleInput for &'a str {
    const TYPE: &'static str = "str";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl RandomOracleInput for Option<Vec<u8>> {
    const TYPE: &'static str = "buffer";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.clone(),
            None => Vec::new(),
        }
    }
}

impl RandomOracleInput for &[u8] {
    const TYPE: &'static str = "buffer";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        self.to_vec()
    }
}

impl RandomOracleInput for Vec<u8> {
    const TYPE: &'static str = "buffer";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        self.clone()
    }
}

impl<const N: usize> RandomOracleInput for [u8; N] {
    const TYPE: &'static str = "buffer";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        Vec::from(self.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use blake3::derive_key;

    use super::*;

    #[test]
    fn test_oracle_compilation() {
        let ro_1 = RandomOracle::empty("test-digest")
            .with("nonce", &202_u64)
            .with("transaction", &"deposit")
            .with_prefix("new".to_string())
            .with("value", &1000_u64)
            .with("type", &"FLK")
            .compile();

        let ro_2 = RandomOracle::empty("test-digest")
            .with("transaction", &"deposit")
            .with("nonce", &202_u64)
            .with_prefix("new".to_string())
            .with("type", &"FLK")
            .with("value", &1000_u64)
            .compile();

        assert_eq!(
            derive_key("test-digest", &ro_1),
            derive_key("test-digest", &ro_2)
        );
    }
}
