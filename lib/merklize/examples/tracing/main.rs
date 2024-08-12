use anyhow::Result;
use atomo::{
    AtomoBuilder,
    DefaultSerdeBackend,
    InMemoryStorage,
    SerdeBackend,
    StorageBackendConstructor,
};
use merklize::hashers::keccak::KeccakHasher;
use merklize::providers::jmt::JmtMerklizeProvider;
use merklize::MerklizeProvider;
use opentelemetry::trace::{TraceError, TracerProvider};
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tracing::trace_span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

#[tokio::main]
async fn main() -> Result<()> {
    let service_name = "merklize-tracing-example";
    let provider = init_tracer_provider(service_name.to_string())?;
    let tracer = provider.tracer(service_name);

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = Registry::default().with(telemetry);

    tracing_subscriber::fmt::init();

    tracing::subscriber::with_default(subscriber, || {
        let span = trace_span!("main");
        let _enter = span.enter();

        let builder = InMemoryStorage::default();
        run::<_, JmtMerklizeProvider<_, DefaultSerdeBackend, KeccakHasher>>(builder, 100);
    });

    Ok(())
}

fn run<B: StorageBackendConstructor, M: MerklizeProvider<Storage = B::Storage>>(
    builder: B,
    data_count: usize,
) {
    let mut db = M::with_tables(AtomoBuilder::new(builder).with_table::<String, String>("data"))
        .build()
        .unwrap();

    // Open writer context and insert some data.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        // Insert some initial data.
        for i in 1..=data_count {
            table.insert(format!("initial-key{i}"), format!("initial-value{i}"));
        }

        // Update state tree.
        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Open writer context and insert some data.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        // Insert data.
        for i in 1..=data_count {
            table.insert(format!("key{i}"), format!("value{i}"));
        }

        // Update state tree.
        M::update_state_tree_from_context(ctx).unwrap();
    });

    // Open reader context, read the data, get the state root hash, and get a proof of existence.
    db.query().run(|ctx| {
        let table = ctx.get_table::<String, String>("data");

        // Read the data.
        let value = table.get("key1".to_string()).unwrap();
        println!("value(key1): {:?}", value);

        // Get the state root hash.
        let root_hash = M::get_state_root(ctx).unwrap();
        println!("state root: {:?}", root_hash);

        // Get a proof of existence for some value in the state.
        let proof = M::get_state_proof(ctx, "data", M::Serde::serialize(&"key1")).unwrap();
        println!("proof: {:?}", proof);
    });
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
