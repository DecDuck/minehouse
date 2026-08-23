#[tarpc::service]
trait MinehouseServer {
    async fn register();
}

#[tarpc::service]
trait MinehouseClient {
    async fn register();
}