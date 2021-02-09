//#![feature(trace_macros)]
#[macro_use]
extern crate slog;
use slog::{Fuse, Logger};

mod common;

fn main() {
    let log = Logger::root(
        Fuse(common::PrintlnDrain),
        o!("version" => "2")
    );

    //trace_macros!(true);
    info!(log, "foo is {foo}", foo = 2; "a" => "b");
    info!(log, "foo is {foo} {bar}", bar=3, foo = 2; "a" => "b");
    info!(log, "foo is {foo} {bar} {baz}", bar=3, foo = 2, baz=4; "a" => "b");
    info!(log, "foo is {foo} {bar} {baz}", bar = 3, foo = 2, baz = 4;);
    info!(log, "foo is {foo} {bar} {baz}", bar=3, foo = 2, baz=4);
    info!(log, "foo is {foo} {bar} {baz}", bar=3, foo = 2, baz=4,);
    info!(log, "formatted {num_entries} entries of {}", "something", num_entries = 2; "log-key" => true);
    info!(log, "{first} {third} {second}", first = 1, second = 2, third=3; "forth" => 4, "fifth" => 5);
}
