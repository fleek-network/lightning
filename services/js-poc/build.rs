use std::sync::Arc;

use ::deno_web::{deno_web, TimersPermission};
use deno_console::deno_console;
use deno_core::extension;
use deno_crypto::deno_crypto;
use deno_url::deno_url;
use deno_webidl::deno_webidl;

struct Permissions {}
impl TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
        unreachable!()
    }
}

fn main() {
    extension!(
        fleek,
        esm_entry_point = "ext:fleek/bootstrap.js",
        esm = [
            dir "src/runtime/js",
            "util.js",
            "fleek.js",
            "global.js",
            "bootstrap.js"
        ]
    );

    let extensions = vec![
        fleek::init_ops_and_esm(),
        deno_webidl::init_ops_and_esm(),
        deno_console::init_ops_and_esm(),
        deno_url::init_ops_and_esm(),
        deno_web::init_ops_and_esm::<Permissions>(Arc::new(Default::default()), None),
        deno_crypto::init_ops_and_esm(None),
    ];

    let _snapshot = deno_core::snapshot_util::create_snapshot(
        deno_core::snapshot_util::CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            snapshot_path: format!("{}/snapshot.bin", std::env::var("OUT_DIR").unwrap()).into(),
            startup_snapshot: None,
            skip_op_registration: false,
            compression_cb: None,
            with_runtime_cb: None,
            extensions,
        },
    );
}
