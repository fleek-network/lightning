use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ::deno_fetch::{deno_fetch, FetchPermissions, Options};
use ::deno_web::{deno_web, TimersPermission};
use anyhow::{anyhow, Result};
use deno_canvas::deno_canvas;
use deno_console::deno_console;
use deno_core::serde_v8::Serializable;
use deno_core::url::Url;
use deno_core::v8::{self, CreateParams, Global, Value};
use deno_core::{JsRuntime, RuntimeOptions, Snapshot};
use deno_crypto::deno_crypto;
use deno_url::deno_url;
use deno_webgpu::deno_webgpu;
use deno_webidl::deno_webidl;
use extensions::fleek;

use self::tape::{Punch, Tape};
use crate::params::{FETCH_BLACKLIST, HEAP_INIT, HEAP_LIMIT};

mod extensions;
mod tape;

/// Snapshot of the runtime after javascript modules have been initialized
static SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/snapshot.bin"));

pub struct Runtime {
    pub deno: JsRuntime,
    tape: Tape,
}

struct Permissions {}
impl TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
        false
    }
}
impl FetchPermissions for Permissions {
    fn check_net_url(
        &mut self,
        url: &Url,
        _api_name: &str,
    ) -> std::prelude::v1::Result<(), deno_core::error::AnyError> {
        if let Some(host) = url.host_str() {
            if FETCH_BLACKLIST.contains(&host) {
                return Err(anyhow!("{host} is blacklisted"));
            }
        }
        Ok(())
    }
    fn check_read(
        &mut self,
        _p: &std::path::Path,
        _api_name: &str,
    ) -> std::prelude::v1::Result<(), deno_core::error::AnyError> {
        // Disable reading files via fetch
        Err(anyhow!("paths are disabled :("))
    }
}

impl Runtime {
    /// Create a new runtime
    pub fn new(mut location: Url) -> Result<Self> {
        let tape = Tape::new(location.clone());
        let mut deno = JsRuntime::new(RuntimeOptions {
            extensions: vec![
                // WebApi subset
                deno_webidl::init_ops(),
                deno_console::init_ops(),
                deno_url::init_ops(),
                deno_web::init_ops::<Permissions>(Arc::new(Default::default()), None),
                deno_fetch::init_ops::<Permissions>(Options::default()),
                deno_crypto::init_ops(None),
                deno_webgpu::init_ops(),
                deno_canvas::init_ops(),
                // Fleek runtime
                fleek::init_ops(),
            ],
            startup_snapshot: Some(Snapshot::Static(SNAPSHOT)),
            op_metrics_factory_fn: Some(tape.op_metrics_factory_fn()),
            // Heap initializes with 1KiB, maxes out at 10MiB
            create_params: Some(CreateParams::default().heap_limits(HEAP_INIT, HEAP_LIMIT)),
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
            let time: v8::Local<v8::Value> = v8::BigInt::new_from_u64(scope, time as u64).into();
            let url = location.to_v8(scope).unwrap();
            let undefined = v8::undefined(scope);

            // Bootstrap.
            bootstrap_fn
                .call(scope, undefined.into(), &[time, url])
                .expect("Failed to execute bootstrap");
        }

        Ok(Self { deno, tape })
    }

    /// Execute javascript source on the runtime
    pub fn exec(&mut self, source: String) -> anyhow::Result<Global<Value>> {
        self.deno.execute_script("<anon>", source.into())
    }

    /// End and collect the punch tape
    pub fn end(self) -> Vec<Punch> {
        self.tape.end()
    }
}
