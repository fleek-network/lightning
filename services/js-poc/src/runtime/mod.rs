use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use deno_core::v8::{CreateParams, Global, Value};
use deno_core::{JsRuntime, RuntimeOptions, Snapshot};
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

impl Runtime {
    /// Create a new runtime with an optionally given startup snapshot
    pub fn new(hash: [u8; 32]) -> Self {
        let tape = Tape::new(hash);
        Self {
            deno: JsRuntime::new(RuntimeOptions {
                extensions: vec![fleek::init_ops()],
                startup_snapshot: Some(Snapshot::Static(SNAPSHOT)),
                op_metrics_factory_fn: Some(tape.op_metrics_factory_fn()),
                // Heap initializes with 1KiB, maxes out at 10MiB
                create_params: Some(CreateParams::default().heap_limits(HEAP_INIT, HEAP_LIMIT)),
                ..Default::default()
            }),
            tape,
        }
    }

    /// Execute javascript source on the runtime
    pub fn exec(&mut self, source: String) -> anyhow::Result<Global<Value>> {
        // Initialize environment
        let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        // TODO: Use a lower level method of disabling access to certain builtin ops.
        self.deno
            .execute_script(
                "<anon>",
                format!("delete Deno.core.ops.op_panic; globalThis.Date.now = () => {time};")
                    .into(),
            )
            .context("failed to execute environment initialization")?;

        self.deno
            .execute_script("<anon>", source.into())
            .context("failed to execute javascript")
    }

    pub fn end(self) -> Vec<Punch> {
        self.tape.end()
    }
}
