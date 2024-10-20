use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ::deno_fetch::{deno_fetch, Options};
use ::deno_net::deno_net;
use ::deno_web::deno_web;
use ::deno_websocket::deno_websocket;
use anyhow::{bail, Result};
use deno_ast::{ParseParams, SourceMapOption};
use deno_canvas::deno_canvas;
use deno_console::deno_console;
use deno_core::error::AnyError;
use deno_core::serde_v8::{self, to_v8};
use deno_core::url::Url;
use deno_core::v8::{self, CreateParams, Global, Value};
use deno_core::{
    JsRuntime,
    ModuleCodeString,
    ModuleName,
    ModuleSpecifier,
    PollEventLoopOptions,
    RuntimeOptions,
    SourceMapData,
};
use deno_crypto::deno_crypto;
use deno_fleek::{fleek, Permissions};
use deno_fs::sync::MaybeArc;
use deno_fs::InMemoryFs;
use deno_media_type::MediaType;
use deno_url::deno_url;
use deno_webgpu::deno_webgpu;
use deno_webidl::deno_webidl;

use self::module_loader::FleekModuleLoader;
use self::tape::{Punch, Tape};
use crate::params::{HEAP_INIT, HEAP_LIMIT};

pub mod guard;
pub mod module_loader;
pub mod tape;

/// Snapshot of the runtime after javascript modules have been initialized
static SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/snapshot.bin"));

pub struct Runtime {
    pub deno: JsRuntime,
    tape: Tape,
}

impl Runtime {
    /// Create a new runtime
    pub fn new(location: Url, depth: u8) -> Result<Self> {
        let memory_fs = MaybeArc::new(InMemoryFs::default());
        let tape = Tape::new(location.clone());
        let mut deno = JsRuntime::new(RuntimeOptions {
            extensions: vec![
                // WebApi subset
                deno_webidl::init_ops(),
                deno_console::init_ops(),
                deno_url::init_ops(),
                deno_web::init_ops::<Permissions>(Arc::new(Default::default()), None),
                deno_net::init_ops::<Permissions>(None, None),
                deno_fetch::init_ops::<Permissions>(Options::default()),
                deno_websocket::init_ops::<Permissions>(Default::default(), None, None),
                deno_crypto::init_ops(None),
                deno_webgpu::init_ops(),
                deno_canvas::init_ops(),
                deno_io::deno_io::init_ops(Some(Default::default())),
                deno_fs::deno_fs::init_ops::<Permissions>(memory_fs.clone()),
                deno_node::deno_node::init_ops::<Permissions>(Default::default(), memory_fs),
                // Fleek runtime
                fleek::init_ops(depth),
            ],
            startup_snapshot: Some(SNAPSHOT),
            op_metrics_factory_fn: Some(tape.op_metrics_factory_fn()),
            create_params: Some(CreateParams::default().heap_limits(HEAP_INIT, HEAP_LIMIT)),
            module_loader: Some(Rc::new(FleekModuleLoader::new())),
            extension_transpiler: Some(Rc::new(|specifier, source| {
                maybe_transpile_source(specifier, source)
            })),
            ..Default::default()
        });

        {
            // Get global scope
            let context = deno.main_context();
            let scope = &mut deno.handle_scope();
            let context_local = v8::Local::new(scope, context);
            let global_obj = context_local.global(scope);

            // Get bootstrap function pointer
            let bootstrap_str =
                v8::String::new_external_onebyte_static(scope, b"bootstrap").unwrap();
            let bootstrap_fn = global_obj
                .get(scope, bootstrap_str.into())
                .expect("Failed to get bootstrap fn");
            let bootstrap_fn = v8::Local::<v8::Function>::try_from(bootstrap_fn).unwrap();

            // Construct parameters
            let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            // TODO: parse directly from u128
            let time: v8::Local<v8::Value> = v8::Number::new(scope, time as f64).into();
            let url = to_v8(scope, location).unwrap();
            let undefined = v8::undefined(scope);

            // Bootstrap.
            bootstrap_fn
                .call(scope, undefined.into(), &[time, url])
                .expect("Failed to execute bootstrap");
        }

        Ok(Self { deno, tape })
    }

    /// Execute javascript source on the runtime
    pub async fn exec(
        &mut self,
        specifier: &ModuleSpecifier,
        param: Option<serde_json::Value>,
    ) -> anyhow::Result<Option<Global<Value>>> {
        let id = self.deno.load_main_es_module(specifier).await?;
        self.deno
            .run_event_loop(PollEventLoopOptions::default())
            .await?;
        self.deno.mod_evaluate(id).await?;

        {
            let main = self.deno.get_module_namespace(id)?;
            let scope = &mut self.deno.handle_scope();
            let scope = &mut v8::TryCatch::new(scope);
            let main_local = v8::Local::new(scope, main);

            // Get bootstrap function pointer
            let main_str = v8::String::new_external_onebyte_static(scope, b"main").unwrap();
            let main_fn = main_local.get(scope, main_str.into()).unwrap();

            if !main_fn.is_function() {
                bail!("expected function main, found {}", main_fn.type_repr());
            }
            let main_fn = v8::Local::<v8::Function>::try_from(main_fn)?;

            // construct parameters
            let param = if let Some(param) = param {
                serde_v8::to_v8(scope, param)?
            } else {
                v8::undefined(scope).into()
            };
            let undefined = v8::undefined(scope);

            // call function and move response into a global ref
            let Some(res) = main_fn.call(scope, undefined.into(), &[param]) else {
                if let Some(exception) = scope.exception() {
                    let error = deno_core::error::JsError::from_v8_exception(scope, exception);
                    return Err(error.into());
                }

                return Ok(None);
            };
            Ok(Some(Global::new(scope, res)))
        }
    }

    /// End and collect the punch tape
    pub fn end(self) -> Vec<Punch> {
        self.tape.end()
    }
}

pub fn maybe_transpile_source(
    name: ModuleName,
    source: ModuleCodeString,
) -> std::result::Result<(ModuleCodeString, Option<SourceMapData>), AnyError> {
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
