mod system {
    pub use actix_rt::System as MySystem;
}

#[actix_rt::main(system = "system::MySystem")]
async fn main() {
    futures_util::future::ready(()).await
}
