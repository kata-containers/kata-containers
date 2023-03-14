extern crate std;

use ParseError;

impl std::error::Error for ParseError {
    fn description(&self) -> &str {
        "UUID parse error"
    }
}
