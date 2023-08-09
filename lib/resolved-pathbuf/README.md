# ResolvedPathBuf

This type is a wrapper around a normal [`PathBuf`](https://doc.rust-lang.org/std/path/struct.PathBuf.html)
that is resolved upon construction. This happens through the
[`resolve_path`](https://crates.io/crates/resolve-path) crate.

Additionally, this wrapper preserves the original path as well, and uses the original when
serialized using [`serde`](https://crates.io/crates/serde). This makes this wrapper a safe
tool when dealing with user provided path. For example either through command line arguments
or configuration values.

Preserving the original allows us to avoid modifying the user configuration when serializing
the configuration back to the disk.
