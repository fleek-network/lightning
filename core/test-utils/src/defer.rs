use std::path::{Path, PathBuf};

/// A very simple type that runs a function on drop.
pub struct Deferred {
    fun: Option<Box<dyn FnOnce() + Send + Sync + 'static>>,
}

impl Deferred {
    pub fn new(fun: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self {
            fun: Some(Box::new(fun)),
        }
    }

    pub fn remove_dir_all(path: impl AsRef<Path>) -> Self {
        let path = PathBuf::from(path.as_ref());
        Self::new(move || {
            let _ = std::fs::remove_dir_all(path);
        })
    }

    pub fn remove_file(path: impl AsRef<Path>) -> Self {
        let path = PathBuf::from(path.as_ref());
        Self::new(move || {
            let _ = std::fs::remove_file(path);
        })
    }
}

impl Drop for Deferred {
    fn drop(&mut self) {
        (self.fun.take().unwrap())();
    }
}
