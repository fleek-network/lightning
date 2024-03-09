use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Info {
    pub name: Option<String>,
    pub description: Option<String>,
    pub producer: Option<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}
