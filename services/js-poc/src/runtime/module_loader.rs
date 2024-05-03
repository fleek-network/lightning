use deno_core::{ModuleSpecifier, StaticModuleLoader};

pub fn node_crypto() -> StaticModuleLoader {
    let source = include_str!("js/node_crypto.js");
    let modules = vec![(ModuleSpecifier::parse("node:crypto").unwrap(), source)];
    StaticModuleLoader::new(modules)
}
