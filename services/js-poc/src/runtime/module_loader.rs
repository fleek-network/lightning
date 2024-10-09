use anyhow::{anyhow, bail, Context};
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
};
use fn_sdk::api::fetch_from_origin;
use fn_sdk::blockstore::ContentHandle;
use tracing::trace;

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
        let import = deno_core::resolve_import(specifier, referrer)?;

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
