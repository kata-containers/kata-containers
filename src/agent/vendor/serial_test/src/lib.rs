//! # serial_test
//! `serial_test` allows for the creation of serialised Rust tests using the [serial](attr.serial.html) attribute
//! e.g.
//! ````
//! #[test]
//! #[serial]
//! fn test_serial_one() {
//!   // Do things
//! }
//!
//! #[test]
//! #[serial]
//! fn test_serial_another() {
//!   // Do things
//! }
//! ````
//! Multiple tests with the [serial](attr.serial.html) attribute are guaranteed to be executed in serial. Ordering
//! of the tests is not guaranteed however.

use lazy_static::lazy_static;
use parking_lot::ReentrantMutex;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};

lazy_static! {
    static ref LOCKS: Arc<RwLock<HashMap<String, ReentrantMutex<()>>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

fn check_new_key(name: &str) {
    // Check if a new key is needed. Just need a read lock, which can be done in sync with everyone else
    let new_key = {
        let unlock = LOCKS.read().unwrap();
        !unlock.deref().contains_key(name)
    };
    if new_key {
        // This is the rare path, which avoids the multi-writer situation mostly
        LOCKS
            .write()
            .unwrap()
            .deref_mut()
            .insert(name.to_string(), ReentrantMutex::new(()));
    }
}

#[doc(hidden)]
pub fn serial_core_with_return<E>(name: &str, function: fn() -> Result<(), E>) -> Result<(), E> {
    check_new_key(name);

    let unlock = LOCKS.read().unwrap();
    // _guard needs to be named to avoid being instant dropped
    let _guard = unlock.deref()[name].lock();
    function()
}

#[doc(hidden)]
pub fn serial_core(name: &str, function: fn()) {
    check_new_key(name);

    let unlock = LOCKS.read().unwrap();
    // _guard needs to be named to avoid being instant dropped
    let _guard = unlock.deref()[name].lock();
    function();
}

#[doc(hidden)]
pub async fn async_serial_core_with_return<E>(
    name: &str,
    fut: impl std::future::Future<Output = Result<(), E>>,
) -> Result<(), E> {
    check_new_key(name);

    let unlock = LOCKS.read().unwrap();
    // _guard needs to be named to avoid being instant dropped
    let _guard = unlock.deref()[name].lock();
    fut.await
}

#[doc(hidden)]
pub async fn async_serial_core(name: &str, fut: impl std::future::Future<Output = ()>) {
    check_new_key(name);

    let unlock = LOCKS.read().unwrap();
    // _guard needs to be named to avoid being instant dropped
    let _guard = unlock.deref()[name].lock();
    fut.await
}

// Re-export #[serial].
#[allow(unused_imports)]
pub use serial_test_derive::serial;
