#[path = "../../tracing/tests/support/mod.rs"]
// path attribute requires referenced module to have same name so allow module inception here
#[allow(clippy::module_inception)]
mod support;
pub use self::support::*;
