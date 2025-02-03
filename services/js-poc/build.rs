use std::rc::Rc;
use std::sync::Arc;

use ::deno_fetch::deno_fetch;
use ::deno_net::deno_net;
use ::deno_websocket::deno_websocket;
use deno_canvas::deno_canvas;
use deno_console::deno_console;
use deno_crypto::deno_crypto;
use deno_fleek::in_memory_fs::InMemoryFs;
use deno_fleek::node_traits::{DisabledNpmChecker, InMemorySysWrapper};
use deno_fleek::{fleek, maybe_transpile_source, Permissions};
use deno_fs::sync::MaybeArc;
use deno_url::deno_url;
use deno_webgpu::deno_webgpu;
use deno_webidl::deno_webidl;

fn main() {
    let memory_fs = MaybeArc::new(InMemoryFs::default());
    let extensions = vec![
        deno_webidl::init_ops_and_esm(),
        deno_console::init_ops_and_esm(),
        deno_url::init_ops_and_esm(),
        deno_web::deno_web::init_ops_and_esm::<Permissions>(Arc::new(Default::default()), None),
        deno_net::init_ops_and_esm::<Permissions>(None, None),
        deno_fetch::init_ops_and_esm::<Permissions>(Default::default()),
        deno_websocket::init_ops_and_esm::<Permissions>(Default::default(), None, None),
        deno_crypto::init_ops_and_esm(None),
        deno_webgpu::init_ops_and_esm(),
        deno_canvas::init_ops_and_esm(),
        deno_io::deno_io::init_ops_and_esm(Some(Default::default())),
        deno_fs::deno_fs::init_ops::<Permissions>(memory_fs.clone()),
        deno_node::deno_node::init_ops_and_esm::<
            Permissions,
            DisabledNpmChecker,
            DisabledNpmChecker,
            InMemorySysWrapper,
        >(None, memory_fs),
        fleek::init_ops_and_esm(0),
    ];

    let snapshot = deno_core::snapshot::create_snapshot(
        deno_core::snapshot::CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            extension_transpiler: Some(Rc::new(|specifier, source| {
                maybe_transpile_source(specifier, source)
            })),
            startup_snapshot: None,
            skip_op_registration: false,
            with_runtime_cb: None,
            extensions,
        },
        None,
    )
    .expect("failed to build snapshot");

    // Rebuild snapshot when any files pulled from fs are modified
    for file in snapshot.files_loaded_during_snapshot {
        println!("cargo::rerun-if-changed={}", file.display())
    }

    let out = std::env::var("OUT_DIR").unwrap();

    // Write snapshot to output dir
    std::fs::write(format!("{out}/snapshot.bin"), snapshot.output)
        .expect("failed to write snapshot");
}
