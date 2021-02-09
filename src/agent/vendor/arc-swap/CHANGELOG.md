# 0.4.7

* Rename the `unstable-weak` to `weak` feature. The support is now available on
  1.45 (currently in beta).

# 0.4.6

* Adjust to `Weak::as_ptr` from std (the weak pointer support, relying on
  unstable features).
* Support running on miri (without some optimizations), so dependencies may run
  miri tests.
* Little optimization when waiting out the contention on write operations.

# 0.4.5

* Added `Guard::from_inner`.

# 0.4.4

* Top-level docs rewrite (less rambling, hopefully more readable).

# 0.4.3

* Fix the `Display` implementation on `Guard` to correctly delegate to the
  underlying `Display` implementation.

# 0.4.2

* The Access functionality ‒ ability to pass a handle to subpart of held data to
  somewhere with the ability to update itself.
* Mapped cache can take `FnMut` as well as `Fn`.

# 0.4.1

* Mapped caches ‒ to allow giving access to parts of config only.

# 0.4.0

* Support for Weak pointers.
* RefCnt implemented for Rc.
* Breaking: Big API cleanups.
  - Peek is gone.
  - Terminology of getting the data unified to `load`.
  - There's only one kind of `Guard` now.
  - Guard derefs to the `Arc`/`Option<Arc>` or similar.
  - `Cache` got moved to top level of the crate.
  - Several now unneeded semi-internal traits and trait methods got removed.
* Splitting benchmarks into a separate sub-crate.
* Minor documentation improvements.

# 0.3.11

* Prevention against UB due to dropping Guards and overflowing the guard
  counter (aborting instead, such problem is very degenerate anyway and wouldn't
  work in the first place).

# 0.3.10

* Tweak slot allocation to take smaller performance hit if some leases are held.
* Increase the number of lease slots per thread to 8.
* Added a cache for faster access by keeping an already loaded instance around.

# 0.3.9

* Fix Send/Sync for Guard and Lease (they were broken in the safe but
  uncomfortable direction ‒ not implementing them even if they could).

# 0.3.8

* `Lease<Option<_>>::unwrap()`, `expect()` and `into_option()` for convenient
  use.

# 0.3.7

* Use the correct `#[deprecated]` syntax.

# 0.3.6

* Another locking store (`PrivateSharded`) to complement the global and private
  unsharded ones.
* Comparison to other crates/approaches in the docs.

# 0.3.5

* Updates to documentation, made it hopefully easier to digest.
* Added the ability to separate gen-locks of one ArcSwapAny from others.
* Some speed improvements by inlining.
* Simplified the `lease` method internally, making it faster in optimistic
  cases.

# 0.3.4

* Another potentially weak ordering discovered (with even less practical effect
  than the previous).

# 0.3.3

* Increased potentially weak ordering (probably without any practical effect).

# 0.3.2

* Documentation link fix.

# 0.3.1

* Few convenience constructors.
* More tests (some randomized property testing).

# 0.3.0

* `compare_and_swap` no longer takes `&Guard` as current as that is a sure way
  to create a deadlock.
* Introduced `Lease` for temporary storage, which doesn't suffer from contention
  like `load`, but doesn't block writes like `Guard`. The downside is it slows
  down with number of held by the current thread.
* `compare_and_swap` and `rcu` uses leases.
* Made the `ArcSwap` as small as the pointer itself, by making the
  shards/counters and generation ID global. This comes at a theoretical cost of
  more contention when different threads use different instances.

# 0.2.0

* Added an `ArcSwapOption`, which allows storing NULL values (as None) as well
  as a valid pointer.
* `compare_and_swap` accepts borrowed `Arc` as `current` and doesn't consume one
  ref count.
* Sharding internal counters, to improve performance on read-mostly contented
  scenarios.
* Providing `peek_signal_safe` as the only async signal safe method to use
  inside signal handlers. This removes the footgun with dropping the `Arc`
  returned from `load` inside a signal handler.

# 0.1.4

* The `peek` method to use the `Arc` inside without incrementing the reference
  count.
* Some more (and hopefully better) benchmarks.

# 0.1.3

* Documentation fix (swap is *not* lock-free in current implementation).

# 0.1.2

* More freedom in the `rcu` and `rcu_unwrap` return types.

# 0.1.1

* `rcu` support.
* `compare_and_swap` support.
* Added some primitive benchmarks.

# 0.1.0

* Initial implementation.
