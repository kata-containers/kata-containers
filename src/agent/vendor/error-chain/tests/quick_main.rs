#![allow(dead_code)]
#[macro_use]
extern crate error_chain;

error_chain!();

mod unit {
    use super::*;
    quick_main!(run);

    fn run() -> Result<()> {
        Ok(())
    }
}

mod i32 {
    use super::*;
    quick_main!(run);

    fn run() -> Result<i32> {
        Ok(1)
    }
}

mod closure {
    use super::*;
    quick_main!(|| -> Result<()> { Ok(()) });
}
