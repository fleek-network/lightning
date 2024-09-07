use tracing_subscriber::prelude::*;
use tracing_subscriber::util::TryInitError;

#[allow(unused)]
pub fn try_init_tracing() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    tracing_subscriber::fmt::try_init()
}

#[allow(unused)]
pub fn init_tracing() {
    try_init_tracing().expect("failed to initialize tracing");
}

#[allow(unused)]
pub fn try_init_tracing_with_tokio_console() -> Result<(), TryInitError> {
    tracing_subscriber::registry()
        .with(console_subscriber::spawn())
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".into()),
        )
        .try_init()
}

#[allow(unused)]
pub fn init_tracing_with_tokio_console() {
    try_init_tracing_with_tokio_console().expect("failed to initialize tracing with tokio console");
}
