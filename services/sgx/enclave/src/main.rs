mod blockstore;

extern crate bytes;

fn main() -> std::io::Result<()> {
    // - startup local load balancer with a wasm runtime on each thread
    // - bind to userspace address for incoming requests from handshake:
    //   - read and verify blockstore content
    //   - submit content and request params to load balancer
    //   - wait for response, sign, and send it

    unimplemented!()
}
