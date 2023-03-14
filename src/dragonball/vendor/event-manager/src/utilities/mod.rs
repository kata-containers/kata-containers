// Helper module for tests utilities.
//
// This module is only compiled with `test_utilities` feature on purpose.
// For production code, we do not want to export this functionality.
// At the same time, we need the test utilities to be public such that they can
// be used by multiple categories of tests. Two examples that deem this module
// necessary are the benchmark tests and the integration tests where the implementations
// of subscribers are shared.
//
// Having this module as a feature comes with a disadvantage that needs to be kept in mind.
// `cargo test` will only work when ran with `--feature test-utilities` (or with --all-features). A
// much nicer way to implement this would've been with a utilities crate that is used as
// `dev-dependencies`. Unfortunately, this is not possible because it would introduce a cyclic
// dependency. The `utilities` module has a dependency on `event-manager` because it needs to
// implement the `EventSubscriber` traits, and `event-manager` has a dependency on utilities so
// that they can be used in tests.
#![doc(hidden)]
pub mod subscribers;
