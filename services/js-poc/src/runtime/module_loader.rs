use deno_core::{ModuleSpecifier, StaticModuleLoader};

pub fn get() -> StaticModuleLoader {
    let source = String::from("export default function(name) { return `hello ${name}`; }");
    let modules = vec![(ModuleSpecifier::parse("memory://crypto").unwrap(), source)];
    StaticModuleLoader::new(modules)
}
