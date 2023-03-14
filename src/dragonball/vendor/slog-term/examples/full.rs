#[macro_use]
extern crate slog;
extern crate slog_term;

use slog::Drain;

mod common;

fn main() {
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();

    let log = slog::Logger::root(drain, o!("version" => "0.5"));

    common::simulate_server(log);
}
