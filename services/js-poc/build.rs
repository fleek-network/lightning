use std::sync::Arc;

use ::deno_fetch::{deno_fetch, FetchPermissions};
use ::deno_web::{deno_web, TimersPermission};
use deno_canvas::deno_canvas;
use deno_console::deno_console;
use deno_core::extension;
use deno_crypto::deno_crypto;
use deno_url::deno_url;
use deno_webgpu::deno_webgpu;
use deno_webidl::deno_webidl;

extension!(
    fleek,
    deps = [
        deno_webidl,
        deno_console,
        deno_url,
        deno_web,
        deno_fetch,
        deno_crypto,
        deno_webgpu,
        deno_canvas
    ],
    esm_entry_point = "ext:fleek/bootstrap.js",
    esm = [
        dir "src/runtime/js",
        "util.js",
        "fleek.js",
        "global.js",
        "bootstrap.js"
    ]
);

struct Permissions {}
impl TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
        unreachable!()
    }
}
impl FetchPermissions for Permissions {
    fn check_net_url(
        &mut self,
        _url: &deno_core::url::Url,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        unreachable!()
    }
    fn check_read(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        unreachable!()
    }
}

fn main() {
    let extensions = vec![
        deno_webidl::init_ops_and_esm(),
        deno_console::init_ops_and_esm(),
        deno_url::init_ops_and_esm(),
        deno_web::init_ops_and_esm::<Permissions>(Arc::new(Default::default()), None),
        deno_fetch::init_ops_and_esm::<Permissions>(Default::default()),
        deno_crypto::init_ops_and_esm(None),
        deno_webgpu::init_ops_and_esm(),
        deno_canvas::init_ops_and_esm(),
        fleek::init_ops_and_esm(),
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
