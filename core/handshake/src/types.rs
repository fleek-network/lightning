pub type Hash = [u8; 32];

pub type Nonce = u64;
pub type Secp256k1AffinePoint = [u8; 33];

pub type Secp256k1PublicKey = Secp256k1AffinePoint;

pub type BlsPublicKey = [u8; 48];
pub type BlsSignature = [u8; 96];
