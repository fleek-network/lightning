use std::fmt::Display;
use std::str::FromStr;

use cid::Cid;
use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

const HTTP_ORIGIN: &str = "http";
const IPFS_ORIGIN: &str = "ipfs";
const B3FS_ORIGIN: &str = "b3fs";

/// An immutable pointer is used as a general address to a content living off Fleek Network.
///
/// It is important that the underlying origins guarantee that the same `uri` is forever linked
/// to the same content. However we do not really care about the underlying source for the
/// persistence, the only guarantee which we build upon is that an origin provider once fed the
/// same path *WILL ALWAYS* return the same content (or explicit errors).
#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
pub struct ImmutablePointer {
    /// The underlying source of the data.
    pub origin: OriginProvider,
    /// The serialized path for the content within the respective origin.
    pub uri: Vec<u8>,
}

impl FromStr for ImmutablePointer {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (origin_ty, uri) = s.split_once('=').ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid immutable pointer syntax",
        ))?;
        let (origin, uri) = match origin_ty {
            HTTP_ORIGIN => (OriginProvider::HTTP, uri.to_string().into_bytes()),
            IPFS_ORIGIN => {
                let cid = Cid::try_from(uri)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                (OriginProvider::IPFS, cid.to_bytes())
            },
            B3FS_ORIGIN => (OriginProvider::B3FS, uri.to_string().into_bytes()),
            ty => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid origin type: {ty}"),
                ));
            },
        };
        Ok(ImmutablePointer { origin, uri })
    }
}

/// This enum represents the origins which we support in the protocol. More can be added as we
/// support more origins in future.
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize, IsVariant,
)]
#[non_exhaustive]
pub enum OriginProvider {
    IPFS,
    HTTP,
    B3FS,
}

impl Display for OriginProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OriginProvider::IPFS => write!(f, "ipfs"),
            OriginProvider::HTTP => write!(f, "http"),
            OriginProvider::B3FS => write!(f, "b3fs"),
        }
    }
}

#[cfg(test)]
mod tests {
    use cid::Cid;

    use crate::{ImmutablePointer, OriginProvider};

    #[test]
    fn test_parse_immutable_pointer() {
        let expected_pointer_http = ImmutablePointer {
            origin: OriginProvider::HTTP,
            uri: b"https://lightning.com".to_vec(),
        };
        let parsed_pointer_http: ImmutablePointer = "http=https://lightning.com".parse().unwrap();
        assert_eq!(expected_pointer_http, parsed_pointer_http);

        let cid =
            Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
        let expected_pointer_ipfs = ImmutablePointer {
            origin: OriginProvider::IPFS,
            uri: cid.to_bytes(),
        };
        let parsed_pointer_ipfs: ImmutablePointer =
            "ipfs=bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
                .parse()
                .unwrap();
        assert_eq!(expected_pointer_ipfs, parsed_pointer_ipfs);

        let expected_pointer_b3fs = ImmutablePointer {
            origin: OriginProvider::B3FS,
            uri: b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_vec(),
        };
        let parsed_pointer_b3fs: ImmutablePointer =
            "b3fs=bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
                .parse()
                .unwrap();
        assert_eq!(expected_pointer_b3fs, parsed_pointer_b3fs);

        assert_eq!(
            "https://lightning.com"
                .parse::<ImmutablePointer>()
                .unwrap_err()
                .to_string(),
            "invalid immutable pointer syntax".to_string()
        );
        assert_eq!(
            "foo=bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
                .parse::<ImmutablePointer>()
                .unwrap_err()
                .to_string(),
            "invalid origin type: foo".to_string()
        );
        assert!("ipfs=bar".parse::<ImmutablePointer>().is_err());
    }
}
