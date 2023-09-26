use zeroize::ZeroizeOnDrop;

pub trait PublicKey: Sized {
    /// The signature associated with this public key.
    type Signature;

    /// Verify a signature against this public key.
    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool;

    /// Encode and return this public key as a string.
    fn to_base58(&self) -> String;

    /// Decode a public key.
    fn from_base58(encoded: &str) -> Option<Self>;
}

pub trait SecretKey: Sized + ZeroizeOnDrop {
    type PublicKey: PublicKey;

    /// Generate a random secret key using a secure source of randomness.
    fn generate() -> Self;

    /// Decode pem data
    fn decode_pem(encoded: &str) -> Option<Self>;

    /// Encode pem data
    fn encode_pem(&self) -> String;

    /// Sign a raw digest of a message.
    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature;

    /// Returns the public key associated with this secret key.
    fn to_pk(&self) -> Self::PublicKey;
}
