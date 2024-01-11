use deno_core::extension;

fn main() {
    extension!(
        fleek,
        esm_entry_point = "ext:fleek/entrypoint.js",
        esm = [ dir "src/runtime", "entrypoint.js" ]
    );

    let _snapshot = deno_core::snapshot_util::create_snapshot(
        deno_core::snapshot_util::CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            snapshot_path: format!("{}/snapshot.bin", std::env::var("OUT_DIR").unwrap()).into(),
            startup_snapshot: None,
            skip_op_registration: false,
            extensions: vec![fleek::init_ops_and_esm()],
            compression_cb: None,
            with_runtime_cb: None,
        },
    );
}
