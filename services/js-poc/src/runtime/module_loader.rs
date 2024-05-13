use deno_core::{ModuleSpecifier, StaticModuleLoader};

pub fn node_modules() -> StaticModuleLoader {
    let crypto_source = include_str!("js/node_crypto.js");
    let zlib_source = include_str!("js/node_zlib.js");
    let https_source = include_str!("js/node_https.js");
    let stream_source = include_str!("js/node_stream.js");
    let path_source = include_str!("js/node_path.js");
    let async_hooks_source = include_str!("js/node_async_hooks.js");
    let modules = vec![
        (
            ModuleSpecifier::parse("node:crypto").unwrap(),
            crypto_source,
        ),
        (ModuleSpecifier::parse("node:zlib").unwrap(), zlib_source),
        (ModuleSpecifier::parse("node:https").unwrap(), https_source),
        (
            ModuleSpecifier::parse("node:stream").unwrap(),
            stream_source,
        ),
        (ModuleSpecifier::parse("node:path").unwrap(), path_source),
        (
            ModuleSpecifier::parse("node:async_hooks").unwrap(),
            async_hooks_source,
        ),
    ];
    StaticModuleLoader::new(modules)
}
