//#![feature(nothreads)]
#[macro_use]
extern crate slog;
use slog::{Fuse, Logger};
use std::cell::RefCell;
use std::rc::Rc;

mod common;

#[derive(Clone)]
struct NoThreadSafeObject {
    val: Rc<RefCell<usize>>,
}

fn main() {
    let log = Logger::root(Fuse(common::PrintlnDrain), o!("version" => "2"));
    let obj = NoThreadSafeObject {
        val: Rc::new(RefCell::new(4)),
    };

    // Move obj2 into a closure. Since it's !Send, this only works
    // with nothreads feature.
    let obj2 = obj.clone();
    let sublog = log.new(o!("obj.val" => slog::FnValue(move |_| {
        format!("{}", obj2.val.borrow())
    })));

    info!(sublog, "test");
}
