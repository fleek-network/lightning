use std::io::Read;
use std::net::TcpStream;

use arrayref::array_ref;
use blake3_tree::blake3::tree::BlockHasher;
use blake3_tree::IncrementalVerifier;

#[allow(unused)]
pub fn get_verified_content(hash: &str) -> anyhow::Result<Vec<u8>> {
    let raw = hex::decode(hash)?;
    let raw = *array_ref![raw, 0, 32];

    let mut stream = TcpStream::connect(&format!("{hash}.blockstore.fleek"))
        .expect("failed to connect to blockstore content stream");

    let mut iv = IncrementalVerifier::new(raw, 0);
    let mut content = Vec::new();

    while !iv.is_done() {
        // read length delimiter
        let mut buf = [0; 4];
        stream.read_exact(&mut buf)?;
        let len = u32::from_be_bytes(buf);

        // read proof or chunk bit
        let mut bit = [0];
        stream.read_exact(&mut bit)?;

        // read payload
        let mut payload = vec![0; len as usize];
        stream.read_exact(&mut payload)?;

        match bit[0] {
            0 => iv.feed_proof(&payload)?,
            1 => {
                // hash block and verify it against the tree
                let mut hasher = BlockHasher::new();
                hasher.set_block(iv.get_current_block_counter());
                hasher.update(&payload);
                iv.verify(hasher)?;

                content.append(&mut payload);
            },
            _ => unreachable!(),
        }
    }

    Ok(content)
}
