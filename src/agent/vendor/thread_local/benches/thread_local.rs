#![feature(test)]

extern crate thread_local;
extern crate test;

use thread_local::{ThreadLocal, CachedThreadLocal};

#[bench]
fn thread_local(b: &mut test::Bencher) {
    let local = ThreadLocal::new();
    b.iter(|| { let _: &i32 = local.get_or(|| Box::new(0)); });
}

#[bench]
fn cached_thread_local(b: &mut test::Bencher) {
    let local = CachedThreadLocal::new();
    b.iter(|| { let _: &i32 = local.get_or(|| Box::new(0)); });
}
