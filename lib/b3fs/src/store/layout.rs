//! The directory layout for the blockstore.
//!
//! The block store is a file based database, we have two (actually three) kind of files that matter
//! to us. The two main ones are *blocks* and *headers*, and the lesser important one is temporary
//! files we write to the disk for atomic moves.
//!
//! The blocks refer to actual segment of a content in the blockstore, and headers are more or less
//! like metadata which contain information about a file based on the root hash of it.
//!
//! For example a directory is represented simply as a header file and it does not have any content
//! so we don't deal with the *blocks* when we deal with a directory. Only header named after the
//! computed hash of a directory (based on its entries) which contains metadata information about
//! it such as a list of entries (could be a **file**, **directory** or a **symbolic link**) in
//! this directory and where they link to. For a symblic link we store some bytes that are meant to
//! be relative path from that directoy. This is of course relative to the root directory that is
//! *mounted*. (We cover this concept in other places)
//!
//! For a directory we also include the hashtree of the merkle tree representing that was merged
//! in order to calculate the root hash of the directoy. This is done and stored in order so that
//! we can generate fast inclusion proofs for random access to a directory entry without needing
//! to perform any hashing again and again. In theory we could avoid storing this information and
//! re-generate it everytime based on the store list of entries, however in practice that would
//! suck - so we don't do it.
//!
//! Now that we talked about directories, we are ready to talk about files. For a file things are
//! a lot simpler and we won't need anything other than a hashtree to describe a file. The leaf
//! nodes of this hash tree would then give us the hash of the segments (aka blocks) of a file.

use std::path::PathBuf;

use rand::Rng;

use crate::utils::to_hex;

pub const BLOCK_DIR_NAME: &'static str = "BLOCKS";
pub const HEADER_DIR_NAME: &'static str = "HEADER";
pub const TEMP_DIR_NAME: &'static str = "TEMP";

/// Return the path to a block file with the provided hash and counter.
pub fn get_block_path(mut root_path: PathBuf, hash: &[u8; 32], counter: usize) -> PathBuf {
    root_path.push(BLOCK_DIR_NAME);
    root_path.push(format!("{}-{}", counter, to_hex(hash)));
    root_path
}

/// Return the path to a header file with the provided hash.
pub fn get_header_path(mut root_path: PathBuf, hash: &[u8; 32]) -> PathBuf {
    root_path.push(HEADER_DIR_NAME);
    root_path.push(to_hex(hash).as_ref());
    root_path
}

/// Return a path to a temp file.
pub fn get_new_temp_file(mut root_path: PathBuf) -> PathBuf {
    let name: [u8; 32] = rand::thread_rng().gen();
    root_path.push(TEMP_DIR_NAME);
    root_path.push(to_hex(&name).as_ref());
    root_path
}
