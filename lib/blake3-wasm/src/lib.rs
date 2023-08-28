use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    let hash = blake3_tree::blake3::hash(name.as_bytes());
    alert(&format!("Hello, {}!", hash.to_hex()));
}
