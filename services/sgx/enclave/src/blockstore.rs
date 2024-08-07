pub fn get_verified_content(hash: &str) -> Vec<u8> {
    let _stream = TcpStream::connect(&format!("{hash}.fleek_blockstore"))
        .expect("failed to connect to blockstore content stream");

    todo!("read and verify response")
}
