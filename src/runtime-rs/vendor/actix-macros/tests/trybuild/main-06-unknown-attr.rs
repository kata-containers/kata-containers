#[actix_rt::main(foo = "bar")]
async fn async_main() {}

#[actix_rt::main(bar::baz)]
async fn async_main2() {}

fn main() {}
