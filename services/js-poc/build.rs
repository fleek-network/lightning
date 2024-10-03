mod ext;

use std::collections::HashMap;
use std::sync::Arc;

use ::deno_fetch::{deno_fetch, FetchPermissions};
use ::deno_net::{deno_net, NetPermissions};
use ::deno_web::{deno_web, TimersPermission};
use ::deno_websocket::{deno_websocket, WebSocketPermissions};
use base64::Engine;
use deno_canvas::deno_canvas;
use deno_console::deno_console;
use deno_core::extension;
use deno_crypto::deno_crypto;
use deno_url::deno_url;
use deno_webgpu::deno_webgpu;
use deno_webidl::deno_webidl;
use serde::Deserialize;

extension!(
    fleek,
    deps = [
        deno_webidl,
        deno_console,
        deno_url,
        deno_web,
        deno_fetch,
        deno_websocket,
        deno_crypto,
        deno_webgpu,
        deno_canvas
    ],
    esm_entry_point = "ext:fleek/bootstrap.js",
    esm = [
        dir "src/runtime/js",
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
impl WebSocketPermissions for Permissions {
    fn check_net_url(
        &mut self,
        _url: &deno_core::url::Url,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        unreachable!()
    }
}
impl NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
        &mut self,
        _host: &(T, Option<u16>),
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

    fn check_write(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        unreachable!()
    }
}

#[derive(Deserialize)]
struct DenoJson {
    imports: HashMap<String, String>,
}

#[derive(Deserialize)]
struct DenoLock {
    remote: HashMap<String, String>,
}

fn create_integrity_url(import: &str, hex_sha256: &str) -> String {
    let deno_hash = hex::decode(hex_sha256).unwrap();
    let sri = base64::engine::general_purpose::STANDARD.encode(deno_hash);
    format!("{import}#integrity=sha256-{sri}")
}

fn main() {
    let extensions = vec![
        deno_webidl::init_ops_and_esm(),
        deno_console::init_ops_and_esm(),
        deno_url::init_ops_and_esm(),
        deno_web::init_ops_and_esm::<Permissions>(Arc::new(Default::default()), None),
        deno_net::init_ops_and_esm::<Permissions>(None, None),
        deno_fetch::init_ops_and_esm::<Permissions>(Default::default()),
        deno_websocket::init_ops_and_esm::<Permissions>(Default::default(), None, None),
        deno_crypto::init_ops_and_esm(None),
        deno_webgpu::init_ops_and_esm(),
        deno_canvas::init_ops_and_esm(),
        fleek::init_ops_and_esm(),
    ];

    let snapshot = deno_core::snapshot::create_snapshot(
        deno_core::snapshot::CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            extension_transpiler: None,
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

    // Read polyfill config and lock
    let config: DenoJson = serde_json::from_reader(
        std::fs::File::open("./ext/node/polyfill/deno.json").expect("failed to read deno.json"),
    )
    .expect("failed to parse deno.json");
    let mut lock: DenoLock = serde_json::from_reader(
        std::fs::File::open("./ext/node/polyfill/deno.lock").expect("failed to read deno.lock"),
    )
    .expect("failed to parse deno.lock");

    // Rebuild if polyfills change
    println!("cargo::rerun-if-changed=./ext/node/polyfill/deno.json");
    println!("cargo::rerun-if-changed=./ext/node/polyfill/deno.lock");

    let mut map = HashMap::new();

    // Insert our top level mappings and take them from the remote sri map
    for (k, v) in config.imports {
        println!("{k}, {v}");
        let complete = create_integrity_url(&v, &lock.remote.remove(&v).unwrap());
        map.insert(k, complete.clone());
        map.insert(v, complete.clone());
    }

    // Insert the remaining remote sri mappings
    for (k, v) in lock.remote {
        let complete = create_integrity_url(&k, &v);
        map.insert(k, complete);
    }

    std::fs::write(
        format!("{out}/importmap.json"),
        serde_json::to_string_pretty(&map).unwrap(),
    )
    .expect("failed to write importmap.json");
}
