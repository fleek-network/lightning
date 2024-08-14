use std::io::Read;
use std::net::TcpStream;

use anyhow::bail;
use arrayref::array_ref;
use blake3_tree::blake3::tree::BlockHasher;
use blake3_tree::IncrementalVerifier;

const LEADING_BIT: u32 = 1 << 31;

/// Get some verified content from the userland.
pub fn get_verified_content(hash: &str) -> anyhow::Result<Vec<u8>> {
    let raw = hex::decode(hash)?;
    if raw.len() != 32 {
        bail!("invalid blake3 hash length");
    }
    let raw = *array_ref![raw, 0, 32];

    let mut stream = TcpStream::connect(&format!("{hash}.blockstore.fleek.network"))
        .expect("failed to connect to blockstore content stream");

    let mut iv = IncrementalVerifier::new(raw, 0);

    // TODO: use userspace memory allocations to avoid having public data
    //       held in the precious and limited protected memory space.
    let mut content = Vec::new();
    let mut block = 0;

    while !iv.is_done() {
        // read leading chunk bit and length delimiter
        let mut buf = [0; 4];
        stream.read_exact(&mut buf)?;
        let mut len = u32::from_be_bytes(buf);
        let is_proof = LEADING_BIT & len == 0;

        // unset leading bit
        len &= !LEADING_BIT;

        println!("reading {len} bytes proof={is_proof}");

        // read payload
        let mut payload = vec![0; len as usize];
        stream.read_exact(&mut payload)?;

        if is_proof {
            iv.feed_proof(&payload)?
        } else {
            // hash block and verify it against the tree
            let mut hasher = BlockHasher::new();
            hasher.set_block(block);
            hasher.update(&payload);
            iv.verify(hasher)?;

            content.append(&mut payload);
            block += 1;
        }
    }

    println!("enclave received verified content");

    Ok(content)
}
