use anyhow::{anyhow, Context};
use arrayref::array_ref;
use cid::Cid;
use deno_core::url::Host;
use deno_core::{
    ModuleLoadResponse,
    ModuleLoader,
    ModuleSource,
    ModuleSourceCode,
    ModuleSpecifier,
    ModuleType,
    RequestedModuleType,
    StaticModuleLoader,
};
use fn_sdk::api::fetch_from_origin;
use fn_sdk::blockstore::ContentHandle;

pub struct FleekModuleLoader {
    static_modules: StaticModuleLoader,
}

impl FleekModuleLoader {
    pub fn new() -> Self {
        Self {
            static_modules: StaticModuleLoader::new(vec![
                (
                    ModuleSpecifier::parse("node:crypto").unwrap(),
                    include_str!("js/node_crypto.js"),
                ),
                (
                    ModuleSpecifier::parse("node:zlib").unwrap(),
                    include_str!("js/node_zlib.js"),
                ),
                (
                    ModuleSpecifier::parse("node:https").unwrap(),
                    include_str!("js/node_https.js"),
                ),
                (
                    ModuleSpecifier::parse("node:stream").unwrap(),
                    include_str!("js/node_stream.js"),
                ),
                (
                    ModuleSpecifier::parse("node:path").unwrap(),
                    include_str!("js/node_path.js"),
                ),
            ]),
        }
    }
}

impl ModuleLoader for FleekModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: deno_core::ResolutionKind,
    ) -> Result<ModuleSpecifier, anyhow::Error> {
        Ok(deno_core::resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&ModuleSpecifier>,
        is_dyn_import: bool,
        requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
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

        // Try loading static modules first
        if let ModuleLoadResponse::Sync(Ok(res)) = self.static_modules.load(
            module_specifier,
            maybe_referrer,
            is_dyn_import,
            requested_module_type,
        ) {
            return ModuleLoadResponse::Sync(Ok(res));
        }

        // If not found, try to load from an origin
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
                let specifier = module_specifier.clone();
                ModuleLoadResponse::Async(Box::pin(async move {
                    let handle = ContentHandle::load(&hash).await?;
                    let source = handle.read_to_end().await?.into_boxed_slice();

                    Ok(ModuleSource::new(
                        module_type,
                        deno_core::ModuleSourceCode::Bytes(source.into()),
                        &specifier,
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

                let specifier = module_specifier.clone();
                ModuleLoadResponse::Async(Box::pin(async move {
                    let hash = fetch_from_origin(fn_sdk::api::Origin::IPFS, cid.to_bytes())
                        .await
                        .context("Failed to fetch ipfs module from origin")?;

                    let handle = ContentHandle::load(&hash).await?;
                    let bytes = handle.read_to_end().await?;

                    let module = ModuleSource::new(
                        module_type,
                        ModuleSourceCode::Bytes(bytes.into_boxed_slice().into()),
                        &specifier,
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

                let specifier = module_specifier.clone();
                ModuleLoadResponse::Async(Box::pin(async move {
                    let hash = fn_sdk::api::fetch_from_origin(
                        fn_sdk::api::Origin::HTTP,
                        specifier.to_string(),
                    )
                    .await
                    .context("failed to fetch http module from origin")?;

                    let handle = ContentHandle::load(&hash).await?;
                    let bytes = handle.read_to_end().await?;

                    let module = ModuleSource::new(
                        module_type,
                        ModuleSourceCode::Bytes(bytes.into_boxed_slice().into()),
                        &specifier,
                        None,
                    );
                    Ok(module)
                }))
            },
            _ => ModuleLoadResponse::Sync(Err(anyhow!("Unknown import url scheme"))),
        }
    }
}
