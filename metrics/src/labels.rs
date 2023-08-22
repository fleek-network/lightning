// const and labels from autometrics-rs
// https://github.com/autometrics-dev/autometrics-rs/blob/main/autometrics/src/labels.rs
// Contants
pub const FUNCTION_KEY: &str = "function";
pub const MODULE_KEY: &str = "module";
pub const CALLER_FUNCTION_KEY: &str = "caller_function";
pub const CALLER_MODULE_KEY: &str = "caller_module";
pub const SERVICE_NAME_KEY: &str = "service_name";
pub const METRICS_SERVICE_NAME: &str = "lightning_metrics";

pub struct Labels {
    function: &'static str,
    module: &'static str,
    service_name: &'static str,
    caller_function: &'static str,
    caller_module: &'static str,
}

pub(crate) type Label = (&'static str, &'static str);

impl Labels {
    pub fn new(function: &'static str, module: &'static str) -> Self {
        Self {
            function,
            module,
            service_name: METRICS_SERVICE_NAME,
            caller_function: "",
            caller_module: "",
        }
    }

    pub fn to_vec(&self) -> Vec<Label> {
        let labels = vec![
            (FUNCTION_KEY, self.function),
            (MODULE_KEY, self.module),
            (SERVICE_NAME_KEY, self.service_name),
            (CALLER_FUNCTION_KEY, self.caller_function),
            (CALLER_MODULE_KEY, self.caller_module),
        ];

        labels
    }
}
