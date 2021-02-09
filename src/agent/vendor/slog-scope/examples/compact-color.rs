#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate slog_scope;

use slog::Drain;
use std::sync::Arc;

mod common;

fn main() {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    let log = slog::Logger::root(Arc::new(drain), o!("version" => "0.5"));

    let _guard = slog_scope::set_global_logger(log);
    common::simulate_server();
}
