#[actix_rt::test(foo = "bar")]
async fn my_test_1() {}

#[actix_rt::test(bar::baz)]
async fn my_test_2() {}

fn main() {}
