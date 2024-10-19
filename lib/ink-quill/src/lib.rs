// inspired by https://gist.github.com/qti3e/ed5b1e06514957f7032d3b41ff362c34

/// Generic trait for implementing digest functionality for an object using a [`TranscriptBuilder`].
///
/// # Example
///
/// ```
/// use ink_quill::{ToDigest, TranscriptBuilder};
///
/// struct ExampleMessage {
///     foo: u64,
///     bar: String,
/// }
///
/// impl ToDigest for ExampleMessage {
///     fn transcript(&self) -> TranscriptBuilder {
///         TranscriptBuilder::empty("example domain")
///             .with("FOO", &self.foo)
///             .with("BAR", &self.bar)
///     }
/// }
///
/// let message = ExampleMessage {
///     foo: 0,
///     bar: "blah".into(),
/// };
/// let digest = message.to_digest();
/// ```
pub trait ToDigest {
    /// Returns the underlying transcript
    fn transcript(&self) -> TranscriptBuilder;

    /// Returns the digest of the object.
    fn to_digest(&self) -> [u8; 32] {
        self.transcript().hash()
    }
}

/// The `TranscriptBuilder` struct is responsible for constructing a transcript of input data.
/// It contains a `domain` for unique identification, a `data` vector to hold the input data,
/// and an optional `prefix` for new labels.
///
/// # Examples
///
/// ```
/// use ink_quill::TranscriptBuilder;
/// let mut builder = TranscriptBuilder::empty("example domain");
/// builder = builder.with("label", &123u64);
/// let result = builder.compile();
/// ```
/// # Panics
///
/// Methods of this struct may panic if called with a duplicated label. To avoid this, ensure
/// labels are unique and do not repeat.
///
/// # Notes
///
/// - The `domain` field is not just for descriptive purposes. It's used as a key in the Blake3
///   hashing function, so changing it will result in different hashes for the same input data.
/// - Methods of this struct may panic if called with a duplicated label. To avoid this, ensure
///   labels are unique and do not repeat.
pub struct TranscriptBuilder {
    domain: String,
    data: Vec<(Vec<u8>, Vec<u8>)>,
    /// A prefix for all the new labels.
    prefix: Option<String>,
}

/// `TranscriptBuilderInput` is a trait that provides an interface for any data
/// that can be passed as input to the `TranscriptBuilder`. Implementers of this trait
/// should provide a unique type identifier and a method to return serialized bytes for the data.
///
/// # Examples
///
/// ```
/// use ink_quill::TranscriptBuilderInput;
///
/// struct MyData {
///     value: u64,
/// }
///
/// impl TranscriptBuilderInput for MyData {
///     const TYPE: &'static str = "MyData";
///
///     fn to_transcript_builder_input(&self) -> Vec<u8> {
///         self.value.to_be_bytes().to_vec()
///     }
/// }
/// ```
pub trait TranscriptBuilderInput {
    /// A unique type identifier of this data type.
    const TYPE: &'static str;

    /// Return serialized bytes for the current data.
    fn to_transcript_builder_input(&self) -> Vec<u8>;
}

impl TranscriptBuilder {
    pub fn empty(domain: impl Into<String>) -> Self {
        TranscriptBuilder {
            domain: domain.into(),
            data: Vec::new(),
            prefix: None,
        }
    }

    pub fn get_domain(&self) -> &str {
        &self.domain
    }

    /// Merges another `TranscriptBuilder` instance into the current one. The function mutates the
    /// current instance, extending its data with the data from the other instance. The function
    /// consumes the other instance and returns the current instance.
    pub fn merge(mut self, other: Self) -> Self {
        for data in other.data {
            self.append_data(data)
        }
        self
    }

    /// Set a prefix for the new values passed to the transcript builder, this can be used when you
    /// want to use the transcript builder in a loop.
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// Provide the builder with the given input and label, the label MUST be unique. The ordering
    /// of the data does not matter.
    pub fn with<T: TranscriptBuilderInput>(mut self, label: &'static str, data: &T) -> Self {
        let mut key = match &self.prefix {
            Some(prefix) => format!("{prefix}/{label}").to_transcript_builder_input(),
            None => label.to_transcript_builder_input(),
        };
        key.extend_from_slice(&(key.len() as u64).to_be_bytes());

        let mut value = data.to_transcript_builder_input();
        value.extend_from_slice(&(value.len() as u64).to_be_bytes());
        value.extend_from_slice(b"/");
        value.extend_from_slice(T::TYPE.as_bytes());
        value.extend_from_slice(b"0");

        let new_pair = (key, value);
        self.append_data(new_pair);

        self
    }

    /// Appends a tuple of vectors of u8 to the data of the TranscriptBuilder. The function sorts
    /// the data vector to ensure that labels are unique.
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

    /// Returns a combined input stream from all the inputs this builder has already
    /// been given.
    pub fn compile(&self) -> Vec<u8> {
        let mut result: Vec<u8> = Vec::new();
        for (key, value) in &self.data {
            result.extend(key);
            result.extend(value);
        }
        result
    }

    /// Returns the hash of the value using the default blake3 hasher.
    pub fn hash(&self) -> [u8; 32] {
        fleek_blake3::derive_key(self.get_domain(), &self.compile())
    }
}

impl TranscriptBuilderInput for Option<i32> {
    const TYPE: &'static str = "i32";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_transcript_builder_input(),
            None => vec![],
        }
    }
}

impl TranscriptBuilderInput for i32 {
    const TYPE: &'static str = "i32";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl TranscriptBuilderInput for Option<i64> {
    const TYPE: &'static str = "i64";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_transcript_builder_input(),
            None => vec![],
        }
    }
}

impl TranscriptBuilderInput for i64 {
    const TYPE: &'static str = "i64";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl TranscriptBuilderInput for u8 {
    const TYPE: &'static str = "u8";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl TranscriptBuilderInput for Option<u8> {
    const TYPE: &'static str = "u8";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_transcript_builder_input(),
            None => vec![],
        }
    }
}

impl TranscriptBuilderInput for u16 {
    const TYPE: &'static str = "u16";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl TranscriptBuilderInput for Option<u16> {
    const TYPE: &'static str = "u16";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_transcript_builder_input(),
            None => vec![],
        }
    }
}

impl TranscriptBuilderInput for Option<u32> {
    const TYPE: &'static str = "u32";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_transcript_builder_input(),
            None => vec![],
        }
    }
}

impl TranscriptBuilderInput for u32 {
    const TYPE: &'static str = "u32";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl TranscriptBuilderInput for u64 {
    const TYPE: &'static str = "u64";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl TranscriptBuilderInput for Option<u64> {
    const TYPE: &'static str = "u64";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_transcript_builder_input(),
            None => vec![],
        }
    }
}

impl TranscriptBuilderInput for u128 {
    const TYPE: &'static str = "u128";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.to_be_bytes())
    }
}

impl TranscriptBuilderInput for Option<u128> {
    const TYPE: &'static str = "u128";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.to_transcript_builder_input(),
            None => vec![],
        }
    }
}

impl TranscriptBuilderInput for String {
    const TYPE: &'static str = "String";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl TranscriptBuilderInput for Option<String> {
    const TYPE: &'static str = "String";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.as_bytes().to_vec(),
            None => Vec::new(),
        }
    }
}

impl<'a> TranscriptBuilderInput for &'a str {
    const TYPE: &'static str = "str";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl TranscriptBuilderInput for Option<Vec<u8>> {
    const TYPE: &'static str = "buffer";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => v.clone(),
            None => Vec::new(),
        }
    }
}

impl TranscriptBuilderInput for &[u8] {
    const TYPE: &'static str = "buffer";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        self.to_vec()
    }
}

impl TranscriptBuilderInput for Vec<u8> {
    const TYPE: &'static str = "buffer";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        self.clone()
    }
}

impl<const N: usize> TranscriptBuilderInput for [u8; N] {
    const TYPE: &'static str = "buffer";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        Vec::from(self.as_ref())
    }
}

impl<const N: usize> TranscriptBuilderInput for Option<[u8; N]> {
    const TYPE: &'static str = "buffer";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Some(v) => Vec::from(v.as_ref()),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use fleek_blake3::derive_key;

    use super::*;

    #[test]
    fn test_builder_compilation() {
        let iq_1 = TranscriptBuilder::empty("test-digest")
            .with("nonce", &202_u64)
            .with("transaction", &"deposit")
            .with_prefix("new".to_string())
            .with("value", &1000_u64)
            .with("type", &"FLK")
            .compile();

        let iq_2 = TranscriptBuilder::empty("test-digest")
            .with("transaction", &"deposit")
            .with("nonce", &202_u64)
            .with_prefix("new".to_string())
            .with("type", &"FLK")
            .with("value", &1000_u64)
            .compile();

        assert_eq!(
            derive_key("test-digest", &iq_1),
            derive_key("test-digest", &iq_2)
        );
    }
}
