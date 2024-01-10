use anyhow::Context;
use deno_core::v8::{Global, Value};
use deno_core::{JsRuntime, RuntimeOptions};
use extensions::fleek;

mod extensions;

pub struct Runtime {
    pub deno: JsRuntime,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            deno: JsRuntime::new(RuntimeOptions {
                extensions: vec![fleek::init_ops_and_esm()],
                ..Default::default()
            }),
        }
    }

    pub fn exec(&mut self, source: String) -> anyhow::Result<Global<Value>> {
        self.deno
            .execute_script("<anon>", source.into())
            .context("failed to execute javascript")
    }
}
