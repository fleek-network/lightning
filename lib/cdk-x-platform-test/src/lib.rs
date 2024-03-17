/// The name of the file that contains the test cases (module)
const CASES: &str = "cases.ts";

#[cfg(test)]
mod tests {
    use super::runtime::Runtime;
    use deno_core::v8;

    #[tokio::test]
    async fn test_encodes_the_same() -> anyhow::Result<()> {
        const CASE: &str = "testEncode";
        /// The encoded comp handhsake
        type Output = Vec<u8>;

        let mut runtime = Runtime::new().await?;
        let res = runtime.try_run::<Output>(CASE).await?;

        let comp = cdk_rust::schema::HandshakeRequestFrame::Handshake { 
            retry: None, 
            service: 1, 
            pk: [1; 96].into(), 
            pop: [2; 48].into() 
        };

        let encoded = comp.encode().to_vec();

        assert_eq!(res, encoded);

        Ok(())
    }

    #[tokio::test]
    async fn test_can_decode_js_from_rust() -> anyhow::Result<()> {
        const CASE: &str = "testDecode";
        // whether or not it decoded correctly
        type Output = bool;

        let comp = cdk_rust::schema::HandshakeRequestFrame::Handshake { 
            retry: None, 
            service: 1, 
            pk: [1; 96].into(), 
            pop: [2; 48].into() 
        };

        let encoded = comp.encode().to_vec();

        let mut runtime = Runtime::new().await?;

        let mut scope = runtime.scope();

        let mut args: Vec<v8::Global<v8::Value>> = Vec::with_capacity(1);
        args.push({
            let local = serde_v8::to_v8(&mut scope, &encoded)?;
            v8::Global::new(&mut scope, local)
        });
        drop(scope);

        let func = runtime.get_func(CASE).await?;
        let res = runtime.call_func_with_args(&func, &args).await?;
        let res = Runtime::deser_res::<Output>(&mut runtime.scope(), res)?;

        assert!(res);

        Ok(())
    }
}

#[cfg(test)]
pub mod runtime {
    use std::cell::OnceCell;
    use std::rc::Rc;

    use anyhow::Context;
    use deno_core::v8::HandleScope;
    use deno_core::{resolve_path, v8, JsRuntime, RuntimeOptions};
    use serde::de::DeserializeOwned;

    use super::resolver::{self, SourceMapStore, TsModuleResolver};
    use super::CASES;

    pub struct Runtime {
        runtime: JsRuntime,
        mod_id: OnceCell<usize>,
    }

    impl Runtime {
        pub async fn try_run<T: DeserializeOwned>(&mut self, case: &str) -> anyhow::Result<T> {
            let func = self.get_func(case).await?;
            let res = self.call_func(&func).await?;

            Self::deser_res(&mut self.runtime.handle_scope(), res)
        }

        async fn load_module(&mut self) -> anyhow::Result<()> {
            let main = resolve_path(
                CASES,
                &std::env::current_dir().context("Unable to get CWD")?,
            )?;

            let mod_id = self.runtime.load_main_es_module(&main).await?;
            self.mod_id.set(mod_id).expect("Module already loaded");

            let result = self.runtime.mod_evaluate(mod_id);
            self.runtime.run_event_loop(Default::default()).await?;
            result.await?;

            Ok(())
        }

        pub async fn get_func(
            &mut self,
            name: impl AsRef<str>,
        ) -> anyhow::Result<v8::Global<v8::Function>> {
            let ns = self.runtime.get_module_namespace(
                self.mod_id.get().cloned().expect("Module not loaded"),
            )?;

            let mut scope = self.runtime.handle_scope();
            let key = v8::String::new(&mut scope, name.as_ref())
                .ok_or(anyhow::anyhow!("Couldnt create function key"))?;

            let func: v8::Local<v8::Function> = ns
                .open(&mut scope)
                .get(&mut scope, key.into())
                .ok_or(anyhow::anyhow!("Couldnt get function from namespace"))?
                .try_into()?;

            Ok(v8::Global::new(&mut scope, func))
        }

        pub async fn call_func(
            &mut self,
            func: &v8::Global<v8::Function>,
        ) -> anyhow::Result<v8::Global<v8::Value>> {
            self.runtime.call(&func).await
        }

        pub async fn call_func_with_args(
            &mut self,
            func: &v8::Global<v8::Function>,
            args: &[v8::Global<v8::Value>],
        ) -> anyhow::Result<v8::Global<v8::Value>> {
            self.runtime.call_with_args(&func, args).await
        }

        pub fn deser_res<'a, T: DeserializeOwned>(
            scope: &mut HandleScope<'a>,
            res: v8::Global<v8::Value>,
        ) -> anyhow::Result<T> {
            let local = v8::Local::new(scope, res);
            Ok(serde_v8::from_v8::<T>(scope, local)?)
        }
    }

    impl Runtime {
        /// Creates the runtime, and loads the test module
        pub async fn new() -> anyhow::Result<Self> {
            let (loader, source_maps) = Self::module_loader();

            let mut this = Self {
                runtime: JsRuntime::new(RuntimeOptions {
                    module_loader: Some(loader),
                    source_map_getter: Some(source_maps),
                    ..Default::default()
                }),
                mod_id: OnceCell::new(),
            };

            this.load_module().await?;

            Ok(this)
        }

        fn module_loader() -> (Rc<TsModuleResolver>, Rc<SourceMapStore>) {
            let (loader, source_maps) = resolver::TsModuleResolver::new();

            (Rc::new(loader), Rc::new(source_maps))
        }

        pub fn scope(&mut self) -> HandleScope {
            self.runtime.handle_scope()
        }
    }
}

#[cfg(test)]
pub mod resolver {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use anyhow::bail;
    use deno_ast::{MediaType, ParseParams, SourceTextInfo};
    use deno_core::{
        ModuleLoadResponse,
        ModuleLoader,
        ModuleSource,
        ModuleSourceCode,
        ModuleSpecifier,
        ModuleType,
        RequestedModuleType,
        SourceMapGetter,
    };

    pub struct TsModuleResolver {
        source_maps: SourceMapStore,
    }

    impl TsModuleResolver {
        pub fn new() -> (Self, SourceMapStore) {
            let source_maps = SourceMapStore(Rc::new(RefCell::new(HashMap::new())));
            (
                Self {
                    source_maps: source_maps.clone(),
                },
                source_maps,
            )
        }
    }

    #[derive(Clone)]
    pub struct SourceMapStore(Rc<RefCell<HashMap<String, Vec<u8>>>>);

    impl SourceMapGetter for SourceMapStore {
        fn get_source_map(&self, specifier: &str) -> Option<Vec<u8>> {
            self.0.borrow().get(specifier).cloned()
        }

        fn get_source_line(&self, _file_name: &str, _line_number: usize) -> Option<String> {
            None
        }
    }

    impl ModuleLoader for TsModuleResolver {
        fn resolve(
            &self,
            specifier: &str,
            referrer: &str,
            _kind: deno_core::ResolutionKind,
        ) -> Result<deno_core::ModuleSpecifier, anyhow::Error> {
            deno_core::resolve_import(specifier, referrer).map_err(|e| e.into())
        }

        fn load(
            &self,
            module_specifier: &ModuleSpecifier,
            _maybe_referrer: Option<&ModuleSpecifier>,
            _is_dyn_import: bool,
            _requested_module_type: RequestedModuleType,
        ) -> ModuleLoadResponse {
            let source_maps = self.source_maps.clone();
            fn load(
                source_maps: SourceMapStore,
                module_specifier: &ModuleSpecifier,
            ) -> anyhow::Result<ModuleSource> {
                let path = module_specifier
                    .to_file_path()
                    .map_err(|_| anyhow::anyhow!("Only file:// URLs are supported."))?;

                let media_type = MediaType::from_path(&path);
                let (module_type, should_transpile) = match MediaType::from_path(&path) {
                    MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                        (ModuleType::JavaScript, false)
                    },
                    MediaType::Jsx => (ModuleType::JavaScript, true),
                    MediaType::TypeScript
                    | MediaType::Mts
                    | MediaType::Cts
                    | MediaType::Dts
                    | MediaType::Dmts
                    | MediaType::Dcts
                    | MediaType::Tsx => (ModuleType::JavaScript, true),
                    MediaType::Json => (ModuleType::Json, false),
                    _ => bail!("Unknown extension {:?}", path.extension()),
                };

                let code = std::fs::read_to_string(&path)?;
                let code = if should_transpile {
                    let parsed = deno_ast::parse_module(ParseParams {
                        specifier: module_specifier.to_string(),
                        text_info: SourceTextInfo::from_string(code),
                        media_type,
                        capture_tokens: false,
                        scope_analysis: false,
                        maybe_syntax: None,
                    })?;
                    let res = parsed.transpile(&deno_ast::EmitOptions {
                        inline_source_map: false,
                        source_map: true,
                        inline_sources: true,
                        ..Default::default()
                    })?;
                    let source_map = res.source_map.unwrap();
                    source_maps
                        .0
                        .borrow_mut()
                        .insert(module_specifier.to_string(), source_map.into_bytes());
                    res.text
                } else {
                    code
                };
                Ok(ModuleSource::new(
                    module_type,
                    ModuleSourceCode::String(code.into()),
                    module_specifier,
                    None,
                ))
            }

            ModuleLoadResponse::Sync(load(source_maps, module_specifier))
        }
    }
}
