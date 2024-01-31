use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ::deno_web::{deno_web, TimersPermission};
use anyhow::{Context, Result};
use deno_console::deno_console;
use deno_core::url::Url;
use deno_core::v8::{self, CreateParams, Global, Value};
use deno_core::{JsRuntime, RuntimeOptions, Snapshot};
use deno_crypto::deno_crypto;
use deno_url::deno_url;
use deno_webidl::deno_webidl;
use extensions::fleek;

use self::tape::{Punch, Tape};
use crate::params::{HEAP_INIT, HEAP_LIMIT};

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

impl Runtime {
    /// Create a new runtime
    pub fn new(hash: [u8; 32], path: Option<String>) -> Result<Self> {
        let mut location = Url::parse(&format!("blake3://{}", hex::encode(hash)))
            .context("failed to create base url")?;
        if let Some(path) = path {
            location = location.join(&path).context("invalid path string")?;
        }

        let tape = Tape::new(hash);
        let mut deno = JsRuntime::new(RuntimeOptions {
            extensions: vec![
                // Fleek sdk
                fleek::init_ops(),
                // WebApi subset
                deno_webidl::init_ops(),
                deno_console::init_ops(),
                deno_url::init_ops(),
                // TODO: why aren't we able to just use maybe_location to init the global scope?
                deno_web::init_ops::<Permissions>(Arc::new(Default::default()), Some(location)),
                deno_crypto::init_ops(None),
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
            let undefined = v8::undefined(scope);

            // Bootstrap.
            bootstrap_fn
                .call(scope, undefined.into(), &[time])
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
