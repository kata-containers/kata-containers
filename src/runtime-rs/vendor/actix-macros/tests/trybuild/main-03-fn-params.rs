#[actix_rt::main]
async fn main2(_param: bool) {
    futures_util::future::ready(()).await
}

fn main() {}
