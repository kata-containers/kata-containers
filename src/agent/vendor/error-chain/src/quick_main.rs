/// Convenient wrapper to be able to use `?` and such in the main. You can
/// use it with a separated function:
///
/// ```
/// # #[macro_use] extern crate error_chain;
/// # error_chain! {}
/// # fn main() {
/// quick_main!(run);
/// # }
///
/// fn run() -> Result<()> {
///     Err("error".into())
/// }
/// ```
///
/// or with a closure:
///
/// ```
/// # #[macro_use] extern crate error_chain;
/// # error_chain! {}
/// # fn main() {
/// quick_main!(|| -> Result<()> {
///     Err("error".into())
/// });
/// # }
/// ```
///
/// You can also set the exit value of the process by returning a type that implements [`ExitCode`](trait.ExitCode.html):
///
/// ```
/// # #[macro_use] extern crate error_chain;
/// # error_chain! {}
/// # fn main() {
/// quick_main!(run);
/// # }
///
/// fn run() -> Result<i32> {
///     Err("error".into())
/// }
/// ```
#[macro_export]
macro_rules! quick_main {
    ($main:expr) => {
        fn main() {
            use std::io::Write;

            ::std::process::exit(match $main() {
                Ok(ret) => $crate::ExitCode::code(ret),
                Err(ref e) => {
                    write!(
                        &mut ::std::io::stderr(),
                        "{}",
                        $crate::ChainedError::display_chain(e)
                    )
                    .expect("Error writing to stderr");

                    1
                }
            });
        }
    };
}

/// Represents a value that can be used as the exit status of the process.
/// See [`quick_main!`](macro.quick_main.html).
pub trait ExitCode {
    /// Returns the value to use as the exit status.
    fn code(self) -> i32;
}

impl ExitCode for i32 {
    fn code(self) -> i32 {
        self
    }
}

impl ExitCode for () {
    fn code(self) -> i32 {
        0
    }
}
