pub async fn main() {
    fn_sdk::ipc::init_from_env();

    // The connection stream.
    // let _stream = fn_sdk::ipc::conn_bind();
    println!("Running io_stress service!");
}
