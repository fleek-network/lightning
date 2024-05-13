use deno_core::{ModuleSpecifier, StaticModuleLoader};

pub fn node_modules() -> StaticModuleLoader {
    let crypto_source = include_str!("js/node_crypto.js");
    let zlib_source = include_str!("js/node_zlib.js");
    let https_source = include_str!("js/node_https.js");
    let modules = vec![
        (
            ModuleSpecifier::parse("node:crypto").unwrap(),
            crypto_source,
        ),
        (ModuleSpecifier::parse("node:zlib").unwrap(), zlib_source),
        (ModuleSpecifier::parse("node:https").unwrap(), https_source),
    ];
    StaticModuleLoader::new(modules)
}
