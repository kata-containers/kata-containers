//! This example demonstrates how to use a `UsersCache` cache in a
//! multi-threaded situation. The cache uses `RefCell`s internally, so it
//! is distinctly not thread-safe. Instead, you’ll need to place it within
//! some kind of lock in order to have threads access it one-at-a-time.
//!
//! It queries all the users it can find in the range 500..510. This is the
//! default uid range on my Apple laptop -- Linux starts counting from 1000,
//! but I can’t include both in the range! It spawns one thread per user to
//! query, with each thread accessing the same cache.
//!
//! Then, afterwards, it retrieves references to the users that had been
//! cached earlier.

// For extra fun, try uncommenting some of the lines of code below, making
// the code try to access the users cache *without* a Mutex, and see it
// spew compile errors at you.

extern crate users;
use users::{Users, UsersCache, uid_t};

extern crate env_logger;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;

const LO: uid_t = 500;
const HI: uid_t = 510;


fn main() {
    env_logger::init();

    // For thread-safely, our users cache needs to be within a Mutex, so
    // only one thread can access it once. This Mutex needs to be within an
    // Arc, so multiple threads can access the Mutex.
    let cache = Arc::new(Mutex::new(UsersCache::new()));
    // let cache = UsersCache::empty_cache();

    // Loop over the range and query all the users in the range. Although we
    // could use the `&User` values returned, we just ignore them.
    for uid in LO .. HI {
        let cache = Arc::clone(&cache);

        thread::spawn(move || {
            let cache = cache.lock().unwrap();  // Unlock the mutex
            let _ = cache.get_user_by_uid(uid); // Query our users cache!
        });
    }

    // Wait for all the threads to finish.
    thread::sleep(Duration::from_millis(100));

    // Loop over the same range and print out all the users we find.
    // These users will be retrieved from the cache.
    for uid in LO .. HI {
        let cache = cache.lock().unwrap();             // Re-unlock the mutex
        if let Some(u) = cache.get_user_by_uid(uid) {  // Re-query our cache!
            println!("User #{} is {}", u.uid(), u.name().to_string_lossy())
        }
        else {
            println!("User #{} does not exist", uid);
        }
    }
}
