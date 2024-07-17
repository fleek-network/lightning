# Blake3 based virtual file system

This crate provides a set of convinient APIs to deal with blake3 files and directories, when
we say 'deal' we are really talking about these common tasks:

1. On-disk storage and retrieval accoring to [Fleek Network](https://fleek.network)'s blockstore.
2. Serving the contents of a blockstore to others.
3. Downloading the content from a remote server with incremental verification.
4. Common utility to mount directories.

And more.