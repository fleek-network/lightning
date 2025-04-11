use arrayref::array_ref;
use cid::Cid;
use deno_core::error::ModuleLoaderError;
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
use deno_error::JsErrorBox;
use fn_sdk::api::fetch_from_origin;
use fn_sdk::blockstore::b3fs::entry::BorrowedLink;
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
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
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
                    return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Unsupported {
                        specifier: Box::new(module_specifier.clone()),
                        maybe_referrer: maybe_referrer.map(|v| Box::new(v.clone())),
                    }));
                }
            },
        };

        let mut module_specifier = module_specifier.clone();
        match module_specifier.scheme() {
            "blake3" => {
                let Some(Host::Domain(host)) = module_specifier.host() else {
                    return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Core(
                        deno_core::error::CoreError::JsBox(JsErrorBox::generic(
                            "missing blake3 hash",
                        )),
                    )));
                };

                let bytes = match hex::decode(host) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Core(
                            deno_core::error::CoreError::JsBox(JsErrorBox::generic(format!(
                                "Invalid blake3 hash: {e}"
                            ))),
                        )));
                    },
                };
                if bytes.len() != 32 {
                    return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Core(
                        deno_core::error::CoreError::JsBox(JsErrorBox::generic(
                            "blake3 hash must be 32 bytes of hex",
                        )),
                    )));
                }

                let hash = *array_ref![bytes, 0, 32];
                ModuleLoadResponse::Async(Box::pin(async move {
                    if !fn_sdk::api::fetch_blake3(hash).await {
                        return Err(ModuleLoaderError::Core(deno_core::error::CoreError::JsBox(
                            JsErrorBox::generic("blake3 hash must be 32 bytes of hex"),
                        )));
                    }

                    let mut handle = ContentHandle::load(&hash).await?;
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
                    return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Core(
                        deno_core::error::CoreError::JsBox(JsErrorBox::generic("missing ipfs cid")),
                    )));
                };
                let Ok(cid) = host.parse::<Cid>() else {
                    return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Core(
                        deno_core::error::CoreError::JsBox(JsErrorBox::generic("invalid ipfs cid")),
                    )));
                };

                ModuleLoadResponse::Async(Box::pin(async move {
                    let Some(hash) =
                        fetch_from_origin(fn_sdk::api::Origin::IPFS, cid.to_bytes()).await
                    else {
                        return Err(ModuleLoaderError::Core(deno_core::error::CoreError::JsBox(
                            JsErrorBox::generic(format!(
                                "failed to fetch {module_specifier} from origin"
                            )),
                        )));
                    };

                    let dir_or_handle = fn_sdk::blockstore::load_dir_or_content(&hash).await?;
                    let bytes = match dir_or_handle {
                        fn_sdk::blockstore::DirOrContent::Dir(mut dir) => {
                            if module_specifier.path().is_empty() {
                                // set default file ahead of time to avoid cannot-be-a-base errors
                                module_specifier.set_path("index.js");
                            }

                            let bucket = fn_sdk::blockstore::blockstore_root().await;
                            let mut segments = module_specifier.path_segments().unwrap().peekable();

                            loop {
                                let mut segment = segments.next().unwrap();
                                if segment.is_empty() {
                                    // if we have an empty segment, there was no final path and we
                                    // should fallback to loading index.js
                                    segment = "index.js";
                                }

                                // load the entry up
                                let Ok(maybe_entry) = dir.get_entry(segment.as_bytes()).await
                                else {
                                    return Err(ModuleLoaderError::Core(
                                        deno_core::error::CoreError::JsBox(JsErrorBox::generic(
                                            format!(
                                                "failed to load dir entries for {module_specifier}"
                                            ),
                                        )),
                                    ));
                                };
                                let Some(entry) = maybe_entry else {
                                    return Err(ModuleLoaderError::Core(
                                        deno_core::error::CoreError::JsBox(JsErrorBox::generic(
                                            format!("file not found in dir for {module_specifier}"),
                                        )),
                                    ));
                                };

                                let hash = match entry.link {
                                    BorrowedLink::Content(hash) => hash,
                                    // TODO: symlink support
                                    BorrowedLink::Path(_) => {
                                        return Err(ModuleLoaderError::Core(
                                            deno_core::error::CoreError::JsBox(
                                                JsErrorBox::generic(
                                                    "symlinks are not supported yet",
                                                ),
                                            ),
                                        ))
                                    },
                                };

                                if segments.peek().is_none() {
                                    // if we are at the final segment, load the file content
                                    let entry = bucket.get(hash).await?;
                                    if entry.is_dir() {
                                        // TODO: should we attempt to load an index.js if the final
                                        //       segment is a directory?
                                        return Err(ModuleLoaderError::Core(
                                            deno_core::error::CoreError::JsBox(
                                                JsErrorBox::generic(
                                                    "expected a file, found directory",
                                                ),
                                            ),
                                        ));
                                    }
                                    let file = entry.into_file().unwrap();
                                    let mut handle = ContentHandle::from_file(bucket, file).await?;

                                    // read the content and return it
                                    break handle.read_to_end().await?;
                                } else {
                                    // otherwise, it is a subdir and should be recursed
                                    let entry = bucket.get(hash).await?;
                                    if entry.is_file() {
                                        return Err(ModuleLoaderError::Core(
                                            deno_core::error::CoreError::JsBox(
                                                JsErrorBox::generic(
                                                    "expected a directory, found file",
                                                ),
                                            ),
                                        ));
                                    }

                                    // store dir for next iteration
                                    dir = entry.into_dir().unwrap();
                                }
                            }
                        },
                        fn_sdk::blockstore::DirOrContent::Content(mut content_handle) => {
                            // the content hash was a file directly, read and return it
                            content_handle.read_to_end().await?
                        },
                    };

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
                    return ModuleLoadResponse::Sync(Err(ModuleLoaderError::Core(
                        deno_core::error::CoreError::JsBox(JsErrorBox::generic(
                            "Missing `#integrity=` subresource identifier fragment",
                        )),
                    )));
                }

                ModuleLoadResponse::Async(Box::pin(async move {
                    let Some(hash) = fn_sdk::api::fetch_from_origin(
                        fn_sdk::api::Origin::HTTP,
                        module_specifier.as_str(),
                    )
                    .await
                    else {
                        return Err(ModuleLoaderError::Core(deno_core::error::CoreError::JsBox(
                            JsErrorBox::generic(format!(
                                "failed to fetch {module_specifier} from origin"
                            )),
                        )));
                    };

                    let mut handle = ContentHandle::load(&hash).await?;
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
            _ => ModuleLoadResponse::Sync(Err(ModuleLoaderError::NotFound)),
        }
    }
}
