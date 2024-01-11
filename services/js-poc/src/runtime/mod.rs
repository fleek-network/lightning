use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use deno_core::v8::{Global, Value};
use deno_core::{JsRuntime, RuntimeOptions, Snapshot};
use extensions::fleek;

mod extensions;

/// Snapshot of the runtime after javascript modules have been initialized
static SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/snapshot.bin"));

pub struct Runtime {
    pub deno: JsRuntime,
}

impl Runtime {
    /// Create a new runtime with an optionally given startup snapshot
    pub fn new() -> Self {
        Self {
            deno: JsRuntime::new(RuntimeOptions {
                extensions: vec![fleek::init_ops()],
                startup_snapshot: Some(Snapshot::Static(SNAPSHOT)),
                ..Default::default()
            }),
        }
    }

    /// Execute javascript source on the runtime
    pub fn exec(&mut self, source: String) -> anyhow::Result<Global<Value>> {
        // Initialize environment
        let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        self.deno
            .execute_script(
                "<anon>",
                format!("globalThis.Date.now = () => {time};").into(),
            )
            .context("failed to execute environment initialization")?;

        self.deno
            .execute_script("<anon>", source.into())
            .context("failed to execute javascript")
    }
}
