use std::rc::Rc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ::deno_fetch::{deno_fetch, FetchPermissions, Options};
use ::deno_net::{deno_net, NetPermissions};
use ::deno_web::{deno_web, TimersPermission};
use ::deno_websocket::{deno_websocket, WebSocketPermissions};
use anyhow::{anyhow, bail, Result};
use deno_canvas::deno_canvas;
use deno_console::deno_console;
use deno_core::serde_v8::{self, to_v8};
use deno_core::url::Url;
use deno_core::v8::{self, CreateParams, Global, Value};
use deno_core::{JsRuntime, ModuleSpecifier, PollEventLoopOptions, RuntimeOptions};
use deno_crypto::deno_crypto;
use deno_url::deno_url;
use deno_webgpu::deno_webgpu;
use deno_webidl::deno_webidl;
use extensions::fleek;

use self::module_loader::FleekModuleLoader;
use self::tape::{Punch, Tape};
use crate::params::{FETCH_BLACKLIST, HEAP_INIT, HEAP_LIMIT};

pub mod extensions;
pub mod module_loader;
pub mod tape;

/// Snapshot of the runtime after javascript modules have been initialized
static SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/snapshot.bin"));

pub struct Runtime {
    pub deno: JsRuntime,
    tape: Tape,
}

struct Permissions {}
impl Permissions {
    fn check_net_url(
        &mut self,
        url: &Url,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        if let Some(host) = url.host_str() {
            if FETCH_BLACKLIST.contains(&host) {
                return Err(anyhow!("{host} is blacklisted"));
            }
        }
        Ok(())
    }
}
impl TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
        false
    }
}
impl FetchPermissions for Permissions {
    fn check_net_url(
        &mut self,
        url: &Url,
        api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        self.check_net_url(url, api_name)
    }
    fn check_read(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        // Disable reading files via fetch
        Err(anyhow!("paths are disabled :("))
    }
}
impl WebSocketPermissions for Permissions {
    fn check_net_url(
        &mut self,
        url: &Url,
        api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        self.check_net_url(url, api_name)
    }
}
impl NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
        &mut self,
        host: &(T, Option<u16>),
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        if FETCH_BLACKLIST.contains(&host.0.as_ref()) {
            Err(anyhow!("{} is blacklisted", host.0.as_ref()))
        } else {
            Ok(())
        }
    }
    fn check_read(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        // Disable reading file descriptors
        Err(anyhow!("paths are disabled :("))
    }
    fn check_write(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
        // Disable writing file descriptors
        Err(anyhow!("paths are disabled :("))
    }
}

impl Runtime {
    /// Create a new runtime
    pub fn new(location: Url, depth: u8) -> Result<Self> {
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
                // Fleek runtime
                fleek::init_ops(depth),
            ],
            startup_snapshot: Some(SNAPSHOT),
            op_metrics_factory_fn: Some(tape.op_metrics_factory_fn()),
            create_params: Some(CreateParams::default().heap_limits(HEAP_INIT, HEAP_LIMIT)),
            module_loader: Some(Rc::new(FleekModuleLoader::new())),
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
