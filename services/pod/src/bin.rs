pub fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive("debug".parse().unwrap())
                .from_env_lossy(),
        )
        .init();

    fleek_service_pod::main()
}
