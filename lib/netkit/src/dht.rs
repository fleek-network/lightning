#[tarpc::service]
pub trait Dht {
    async fn find_node(key: String) -> String;
    async fn find_value(value: String) -> String;
    async fn ping() -> usize;
    async fn store(key: String, value: String) -> String;
}
