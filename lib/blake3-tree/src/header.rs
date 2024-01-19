//! This module contains the implementation of the on-disk representation of a content header.
//!
//! A content can be either a file or a directory. In case of a directory we need to store the
//! name of the entries along with their link as well as the internal hash tree.
//!
//! For the sake of portability this module should be as self-contained as possible.

use crate::{utils::HashTree, directory::Directory};

pub enum Header {
    File(HashTree),
    Directory(Directory)
}

// header:
//  | file
//  | dir
//
// file: Tx32b
//
// dir:
//  Nx[name, link]
//  Tx32b
//
// name: Lx1b
//
// link:
//  | link_file
//  | link_dir
//  | link_symlink
// link_file: 32b
// link_dir: 32b
// link_symlink: Lx1b

impl Header {}