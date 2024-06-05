use std::env;
use std::path::PathBuf;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref LIGHTNING_TEST_HOME_DIR: PathBuf = env::var("LIGHTNING_TEST_HOME")
        .unwrap_or("~/.lightning-test".to_string())
        .into();
}
