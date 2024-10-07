mod ext;

#[path = "src/runtime/rt/mod.rs"]
mod rt;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use ::deno_fetch::{deno_fetch, FetchPermissions};
use ::deno_net::{deno_net, NetPermissions};
use ::deno_web::TimersPermission;
use ::deno_websocket::{deno_websocket, WebSocketPermissions};
use base64::Engine;
use deno_ast::{ParseParams, SourceMapOption};
use deno_canvas::deno_canvas;
use deno_console::deno_console;
use deno_core::error::{AnyError, JsError};
use deno_core::url::Url;
use deno_core::{
    extension,
    op2,
    v8,
    ByteString,
    JsBuffer,
    ModuleCodeString,
    ModuleName,
    OpState,
    ResourceId,
    SourceMapData,
};
use deno_crypto::deno_crypto;
use deno_fs::sync::MaybeArc;
use deno_fs::{FsPermissions, InMemoryFs};
use deno_io::fs::FsError;
use deno_media_type::MediaType;
use deno_napi::NapiPermissions;
use deno_node::NodePermissions;
use deno_permissions::ChildPermissionsArg;
use deno_url::deno_url;
use deno_web::JsMessageData;
use deno_webgpu::deno_webgpu;
use deno_webidl::deno_webidl;
use serde::{Deserialize, Serialize};
use crate::rt::extensions::fleek;
use crate::rt::permissions::Permissions;
use crate::rt::shared::{
    op_bootstrap_color_depth,
    op_bootstrap_unstable_args,
    op_can_write_vectored,
    op_create_worker,
    op_host_post_message,
    op_host_recv_ctrl,
    op_host_recv_message,
    op_host_terminate_worker,
    op_http_set_response_trailers,
    op_napi_open,
    op_raw_write_vectored,
    op_set_raw,
};
//
// extension!(
//     fleek,
//     deps = [
//         deno_webidl,
//         deno_console,
//         deno_url,
//         deno_web,
//         deno_fetch,
//         deno_websocket,
//         deno_crypto,
//         deno_webgpu,
//         deno_canvas,
//         deno_io,
//         deno_fs,
//         deno_node
//     ],
//     parameters = [P: NapiPermissions],
//     ops = [
//         op_set_raw,
//         op_can_write_vectored,
//         op_raw_write_vectored,
//         op_bootstrap_unstable_args,
//         op_http_set_response_trailers,
//         op_bootstrap_color_depth,
//         op_create_worker,
//         op_host_post_message,
//         op_host_recv_ctrl,
//         op_host_recv_message,
//         op_host_terminate_worker,
//         op_napi_open<P>
//     ],
//     esm_entry_point = "ext:fleek/bootstrap.js",
//     esm = [
//         dir "src/runtime/js",
//         "fleek.js",
//         "global.js",
//         "bootstrap.js",
//         "ext:runtime/98_global_scope_shared.js" = "98_global_scope_shared.js",
//         "ext:deno_http/00_serve.ts" = "00_serve.ts",
//         "ext:runtime/30_os.js" = "30_os.js",
//         "ext:deno_broadcast_channel/01_broadcast_channel.js" = "01_broadcast_channel.js",
//         "ext:deno_fs/30_fs.js" = "30_fs.js"
//     ]
// );
//
// struct Permissions {}
// impl TimersPermission for Permissions {
//     fn allow_hrtime(&mut self) -> bool {
//         unreachable!()
//     }
// }
// impl FetchPermissions for Permissions {
//     fn check_net_url(
//         &mut self,
//         _url: &deno_core::url::Url,
//         _api_name: &str,
//     ) -> Result<(), deno_core::error::AnyError> {
//         unreachable!()
//     }
//     fn check_read(
//         &mut self,
//         _p: &std::path::Path,
//         _api_name: &str,
//     ) -> Result<(), deno_core::error::AnyError> {
//         unreachable!()
//     }
// }
// impl WebSocketPermissions for Permissions {
//     fn check_net_url(
//         &mut self,
//         _url: &deno_core::url::Url,
//         _api_name: &str,
//     ) -> Result<(), deno_core::error::AnyError> {
//         unreachable!()
//     }
// }
// impl NetPermissions for Permissions {
//     fn check_net<T: AsRef<str>>(
//         &mut self,
//         _host: &(T, Option<u16>),
//         _api_name: &str,
//     ) -> Result<(), deno_core::error::AnyError> {
//         unreachable!()
//     }
//
//     fn check_read(
//         &mut self,
//         _p: &std::path::Path,
//         _api_name: &str,
//     ) -> Result<(), deno_core::error::AnyError> {
//         unreachable!()
//     }
//
//     fn check_write(
//         &mut self,
//         _p: &std::path::Path,
//         _api_name: &str,
//     ) -> Result<(), deno_core::error::AnyError> {
//         unreachable!()
//     }
// }
//
// impl NodePermissions for Permissions {
//     fn check_net_url(&mut self, url: &Url, api_name: &str) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_read_with_api_name(
//         &mut self,
//         path: &Path,
//         api_name: Option<&str>,
//     ) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_sys(&mut self, kind: &str, api_name: &str) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_write_with_api_name(
//         &mut self,
//         path: &Path,
//         api_name: Option<&str>,
//     ) -> Result<(), AnyError> {
//         todo!()
//     }
// }
//
// impl FsPermissions for Permissions {
//     fn check_open<'a>(
//         &mut self,
//         resolved: bool,
//         read: bool,
//         write: bool,
//         path: &'a Path,
//         api_name: &str,
//     ) -> Result<Cow<'a, Path>, FsError> {
//         todo!()
//     }
//
//     fn check_read(&mut self, path: &Path, api_name: &str) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_read_all(&mut self, api_name: &str) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_read_blind(
//         &mut self,
//         p: &Path,
//         display: &str,
//         api_name: &str,
//     ) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_write(&mut self, path: &Path, api_name: &str) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_write_partial(&mut self, path: &Path, api_name: &str) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_write_all(&mut self, api_name: &str) -> Result<(), AnyError> {
//         todo!()
//     }
//
//     fn check_write_blind(
//         &mut self,
//         p: &Path,
//         display: &str,
//         api_name: &str,
//     ) -> Result<(), AnyError> {
//         todo!()
//     }
// }
//
// impl NapiPermissions for Permissions {
//     fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError> {
//         todo!()
//     }
// }

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
        deno_web::deno_web::init_ops_and_esm::<Permissions>(Arc::new(Default::default()), None),
        deno_net::init_ops_and_esm::<Permissions>(None, None),
        deno_fetch::init_ops_and_esm::<Permissions>(Default::default()),
        deno_websocket::init_ops_and_esm::<Permissions>(Default::default(), None, None),
        deno_crypto::init_ops_and_esm(None),
        deno_webgpu::init_ops_and_esm(),
        deno_canvas::init_ops_and_esm(),
        deno_io::deno_io::init_ops_and_esm(None),
        deno_fs::deno_fs::init_ops::<Permissions>(MaybeArc::new(InMemoryFs::default())),
        deno_node::deno_node::init_ops_and_esm::<Permissions>(
            None,
            MaybeArc::new(InMemoryFs::default()),
        ),
        fleek::init_ops_and_esm::<Permissions>(0),
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

pub fn maybe_transpile_source(
    name: ModuleName,
    source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), AnyError> {
    // Always transpile `node:` built-in modules, since they might be TypeScript.
    let media_type = if name.starts_with("node:") {
        MediaType::TypeScript
    } else {
        MediaType::from_path(Path::new(&name))
    };

    match media_type {
        MediaType::TypeScript => {},
        MediaType::JavaScript => return Ok((source, None)),
        MediaType::Mjs => return Ok((source, None)),
        _ => panic!(
            "Unsupported media type for snapshotting {media_type:?} for file {}",
            name
        ),
    }

    let parsed = deno_ast::parse_module(ParseParams {
        specifier: deno_core::url::Url::parse(&name).unwrap(),
        text: source.into(),
        media_type,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })?;
    let transpiled_source = parsed
        .transpile(
            &deno_ast::TranspileOptions {
                imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
                ..Default::default()
            },
            &deno_ast::EmitOptions {
                source_map: if cfg!(debug_assertions) {
                    SourceMapOption::Separate
                } else {
                    SourceMapOption::None
                },
                ..Default::default()
            },
        )?
        .into_source();

    let maybe_source_map: Option<SourceMapData> = transpiled_source.source_map.map(|sm| sm.into());
    let source_text = String::from_utf8(transpiled_source.source)?;

    Ok((source_text.into(), maybe_source_map))
}
