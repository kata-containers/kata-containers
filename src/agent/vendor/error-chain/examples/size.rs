#[macro_use]
extern crate error_chain;

use std::mem::{size_of, size_of_val};

error_chain! {
    errors {
        AVariant
        Another
    }
}

fn main() {
    println!("Memory usage in bytes");
    println!("---------------------");
    println!("Result<()>: {}", size_of::<Result<()>>());
    println!("  (): {}", size_of::<()>());
    println!("  Error: {}", size_of::<Error>());
    println!("    ErrorKind: {}", size_of::<ErrorKind>());
    let msg = ErrorKind::Msg("test".into());
    println!("      ErrorKind::Msg: {}", size_of_val(&msg));
    println!("        String: {}", size_of::<String>());
    println!("    State: {}", size_of::<error_chain::State>());
    let state = error_chain::State {
        next_error: None,
        backtrace: error_chain::InternalBacktrace::new(),
    };
    println!("      State.next_error: {}", size_of_val(&state.next_error));
    println!("      State.backtrace: {}", size_of_val(&state.backtrace));
}
