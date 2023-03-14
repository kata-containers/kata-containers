#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_json;

use slog::Drain;

fn main() {
    let drain = slog_json::Json::new(std::io::stdout())
        .set_pretty(true)
        .add_default_keys()
        .build()
        .fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let log = slog::Logger::root(drain, o!("format" => "pretty"));

    info!(log, "An example log message"; "foo" => "bar");
    info!(log, "Another example log message"; "fizz" => "buzz");
}
