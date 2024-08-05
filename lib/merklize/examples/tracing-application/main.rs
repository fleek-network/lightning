use anyhow::Result;
use atomo::DefaultSerdeBackend;
use futures::executor::block_on;
use lightning_application::storage::AtomoStorage;
use merklize::hashers::keccak::KeccakHasher;
use merklize::providers::jmt::JmtMerklizeProvider;
use merklize_test_utils::application::{
    create_rocksdb_env,
    new_complex_block,
    BaselineMerklizeProvider,
    DummyPutter,
};
use opentelemetry::trace::{TraceError, TracerProvider};
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tempfile::tempdir;
use tracing::trace_span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

#[tokio::main]
async fn main() -> Result<()> {
    let service_name = "merklize-tracing-application-example";
    let provider = init_tracer_provider(service_name.to_string())?;
    let tracer = provider.tracer(service_name);

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = Registry::default().with(telemetry);

    tracing_subscriber::fmt::init();

    tracing::subscriber::with_default(subscriber, || {
        block_on(async {
            let span = trace_span!("main");
            let _enter = span.enter();

            {
                let span = trace_span!("baseline");
                let _enter = span.enter();

                let temp_dir = tempdir().unwrap();
                let mut env = create_rocksdb_env::<BaselineMerklizeProvider>(&temp_dir);
                let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();

                env.run(block.clone(), || DummyPutter {}).await;
            }

            {
                let span = trace_span!("jmt");
                let _enter = span.enter();

                let temp_dir = tempdir().unwrap();
                let mut env = create_rocksdb_env::<
                    JmtMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>,
                >(&temp_dir);
                let (block, _stake_amount, _eth_addresses, _node_public_keys) = new_complex_block();

                env.run(block.clone(), || DummyPutter {}).await;
            }
        });
    });

    Ok(())
}

fn init_tracer_provider(service_name: String) -> Result<sdktrace::TracerProvider, TraceError> {
    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317"),
        )
        .with_trace_config(
            sdktrace::Config::default().with_resource(Resource::new(vec![KeyValue::new(
                SERVICE_NAME,
                service_name,
            )])),
        )
        .install_batch(runtime::Tokio)
}
