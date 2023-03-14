mod system {
    pub use actix_rt::System as MySystem;
}

#[actix_rt::test(system = "system::MySystem")]
async fn my_test() {
    futures_util::future::ready(()).await
}

fn main() {}
