use core::fmt::{self, Display, Debug};

use Fail;
use Error;

/// Constructs a `Fail` type from a string.
///
/// This is a convenient way to turn a string into an error value that
/// can be passed around, if you do not want to create a new `Fail` type for
/// this use case.
pub fn err_msg<D: Display + Debug + Sync + Send + 'static>(msg: D) -> Error {
    Error::from(ErrorMessage { msg })
}

/// A `Fail` type that just contains an error message. You can construct
/// this from the `err_msg` function.
#[derive(Debug)]
struct ErrorMessage<D: Display + Debug + Sync + Send + 'static> {
    msg: D,
}

impl<D: Display + Debug + Sync + Send + 'static> Fail for ErrorMessage<D> {
    fn name(&self) -> Option<&str> {
        Some("failure::ErrorMessage")
    }
}

impl<D: Display + Debug + Sync + Send + 'static> Display for ErrorMessage<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.msg, f)
    }
}
