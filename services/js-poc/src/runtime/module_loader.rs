use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context};
use arrayref::array_ref;
use cid::Cid;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::StreamExt;
use deno_core::url::Host;
use deno_core::{
    ModuleLoadResponse,
    ModuleLoader,
    ModuleSource,
    ModuleSourceCode,
    ModuleSpecifier,
    ModuleType,
    RequestedModuleType,
};
use fn_sdk::api::fetch_from_origin;
use fn_sdk::blockstore::ContentHandle;
use tokio::sync::Semaphore;
use tracing::{debug, trace, warn};

static IMPORTS: OnceLock<HashMap<ModuleSpecifier, ModuleSpecifier>> = OnceLock::new();

// Initialize the module loader
pub fn get_or_init_imports<'a>() -> &'a HashMap<ModuleSpecifier, ModuleSpecifier> {
    IMPORTS.get_or_init(|| {
        let map: HashMap<ModuleSpecifier, ModuleSpecifier> =
            serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/importmap.json")))
                .unwrap();

        // Spawn a task to prefetch the imports
        let to_fetch = map.clone();
        tokio::spawn(async move {
            // Limit number of concurrent requests
            let semaphore = Semaphore::new(16);
            if to_fetch
                .values()
                .map(|uri| async {
                    let _ = semaphore.acquire().await.ok()?;
                    let uri = uri.as_str();
                    let hash = fetch_from_origin(fn_sdk::api::Origin::HTTP, uri).await;
                    trace!("Fetched {uri} from origin");
                    hash
                })
                .collect::<FuturesUnordered<_>>()
                .any(|res| async move { res.is_none() })
                .await
            {
                warn!("Failed to prefetch runtime imports");
            } else {
                debug!("Prefetched runtime imports successfully")
            }
        });

        map
    })
}

pub struct FleekModuleLoader {}

impl FleekModuleLoader {
    pub fn new() -> Self {
        Self {}
    }
}

impl ModuleLoader for FleekModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: deno_core::ResolutionKind,
    ) -> Result<ModuleSpecifier, anyhow::Error> {
        // Resolve import according to spec, reusing referrer base urls, etc
        let mut import = deno_core::resolve_import(specifier, referrer)?;

        // If we have an override in our importmap, use it instead
        if let Some(mapped) = get_or_init_imports().get(&import) {
            import = mapped.clone();
        }

        Ok(import)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        trace!(
            "LOAD specifier: {module_specifier} maybe_referrer {}",
            maybe_referrer.map(|m| m.as_str()).unwrap_or("none")
        );

        // Manually override module
        if module_specifier.as_str() == "node:util" {
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(
                    include_str!("../../polyfill/overrides/util.js")
                        .to_string()
                        .into(),
                ),
                module_specifier,
                None,
            )));
        }

        let is_top_level_module = maybe_referrer.is_none();

        let module_type = match requested_module_type {
            RequestedModuleType::None => ModuleType::JavaScript,
            RequestedModuleType::Json => ModuleType::Json,
            RequestedModuleType::Other(ref t) => {
                if t.to_lowercase() == "wasm" {
                    ModuleType::Wasm
                } else {
                    return ModuleLoadResponse::Sync(Err(anyhow!("Unknown requested module type")));
                }
            },
        };

        let module_specifier = module_specifier.clone();
        match module_specifier.scheme() {
            "blake3" => {
                let Some(Host::Domain(host)) = module_specifier.host() else {
                    return ModuleLoadResponse::Sync(Err(anyhow!("Invalid blake3 hash")));
                };

                let bytes = match hex::decode(host) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return ModuleLoadResponse::Sync(Err(anyhow!("Invalid blake3 hash: {e}")));
                    },
                };
                if bytes.len() != 32 {
                    return ModuleLoadResponse::Sync(Err(anyhow!(
                        "Invalid blake3 hash: length must be 32 bytes"
                    )));
                }

                let hash = *array_ref![bytes, 0, 32];
                ModuleLoadResponse::Async(Box::pin(async move {
                    if !fn_sdk::api::fetch_blake3(hash).await {
                        bail!("Failed to fetch {module_specifier}")
                    }

                    if is_top_level_module {
                        // Submit the hash of the executed js
                        fn_sdk::api::submit_js_hash(1, hash).await;
                    }

                    let handle = ContentHandle::load(&hash).await?;
                    let source = handle.read_to_end().await?.into_boxed_slice();

                    Ok(ModuleSource::new(
                        module_type,
                        deno_core::ModuleSourceCode::Bytes(source.into()),
                        &module_specifier,
                        None,
                    ))
                }))
            },
            "ipfs" => {
                let Some(Host::Domain(host)) = module_specifier.host() else {
                    return ModuleLoadResponse::Sync(Err(anyhow!("Invalid ipfs cid")));
                };
                let Ok(cid) = host.parse::<Cid>() else {
                    return ModuleLoadResponse::Sync(Err(anyhow!("Invalid ipfs cid")));
                };

                ModuleLoadResponse::Async(Box::pin(async move {
                    let hash = fetch_from_origin(fn_sdk::api::Origin::IPFS, cid.to_bytes())
                        .await
                        .with_context(|| {
                            format!("Failed to fetch {module_specifier} from origin")
                        })?;

                    if is_top_level_module {
                        // Submit the hash of the executed js
                        fn_sdk::api::submit_js_hash(1, hash).await;
                    }

                    let handle = ContentHandle::load(&hash).await?;
                    let bytes = handle.read_to_end().await?;

                    let module = ModuleSource::new(
                        module_type,
                        ModuleSourceCode::Bytes(bytes.into_boxed_slice().into()),
                        &module_specifier,
                        None,
                    );
                    Ok(module)
                }))
            },
            "https" | "http" => {
                if !module_specifier
                    .fragment()
                    .map(|s| s.starts_with("integrity="))
                    .unwrap_or(false)
                {
                    return ModuleLoadResponse::Sync(Err(anyhow!(
                        "Missing `#integrity=` subresource identifier fragment"
                    )));
                }

                ModuleLoadResponse::Async(Box::pin(async move {
                    let hash = fn_sdk::api::fetch_from_origin(
                        fn_sdk::api::Origin::HTTP,
                        module_specifier.as_str(),
                    )
                    .await
                    .with_context(|| format!("Failed to fetch {module_specifier} from origin"))?;

                    if is_top_level_module {
                        // Submit the hash of the executed js
                        fn_sdk::api::submit_js_hash(1, hash).await;
                    }

                    let handle = ContentHandle::load(&hash).await?;
                    let bytes = handle.read_to_end().await?;

                    let module = ModuleSource::new(
                        module_type,
                        ModuleSourceCode::Bytes(bytes.into_boxed_slice().into()),
                        &module_specifier,
                        None,
                    );
                    Ok(module)
                }))
            },
            _ => ModuleLoadResponse::Sync(Err(anyhow!("Unknown import url scheme"))),
        }
    }
}
