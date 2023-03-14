//! Checks that test macro does not cause problems in the presence of imports named "test" that
//! could be either a module with test items or the "test with runtime" macro itself.
//!
//! Before actix/actix-net#399 was implemented, this macro was running twice. The first run output
//! `#[test]` and it got run again and since it was in scope.
//!
//! Prevented by using the fully-qualified test marker (`#[::core::prelude::v1::test]`).

#![cfg(feature = "macros")]

use actix_rt::time as test;

#[actix_rt::test]
async fn test_naming_conflict() {
    use test as time;
    time::sleep(std::time::Duration::from_millis(2)).await;
}
