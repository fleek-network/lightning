use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use deno_core::v8::{Global, Value};
use deno_core::{JsRuntime, JsRuntimeForSnapshot, PollEventLoopOptions, RuntimeOptions, Snapshot};
use extensions::fleek;

mod extensions;

pub struct Runtime {
    pub deno: JsRuntime,
}

impl Runtime {
    /// Create a new runtime with an optionally given startup snapshot
    pub fn new(startup_snapshot: Option<Box<[u8]>>) -> Self {
        Self {
            deno: JsRuntime::new(build_options(startup_snapshot.map(Snapshot::Boxed))),
        }
    }

    /// Create a new startup snapshot of the runtime
    pub async fn snapshot() -> Result<Box<[u8]>> {
        let mut rt = JsRuntimeForSnapshot::new(build_options(None));
        rt.run_event_loop2(PollEventLoopOptions::default()).await?;
        Ok(rt.snapshot().to_vec().into_boxed_slice())
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

fn build_options(startup_snapshot: Option<Snapshot>) -> RuntimeOptions {
    RuntimeOptions {
        extensions: match startup_snapshot.is_none() {
            // esm modules should only be initialized during snapshot creation
            true => vec![fleek::init_ops_and_esm()],
            false => vec![fleek::init_ops()],
        },
        startup_snapshot,
        ..Default::default()
    }
}
