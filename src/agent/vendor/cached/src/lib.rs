/*!
[![Build Status](https://github.com/jaemk/cached/actions/workflows/build.yml/badge.svg)](https://github.com/jaemk/cached/actions/workflows/build.yml)
[![crates.io](https://img.shields.io/crates/v/cached.svg)](https://crates.io/crates/cached)
[![docs](https://docs.rs/cached/badge.svg)](https://docs.rs/cached)

> Caching structures and simplified function memoization

`cached` provides implementations of several caching structures as well as a handy macros
for defining memoized functions.

Memoized functions defined using [`#[cached]`](proc_macro::cached)/[`#[once]`](proc_macro::once)/[`#[io_cached]`](proc_macro::io_cached)/[`cached!`](crate::macros) macros are thread-safe with the backing
function-cache wrapped in a mutex/rwlock, or externally synchronized in the case of `#[io_cached]`.
By default, the function-cache is **not** locked for the duration of the function's execution, so initial (on an empty cache)
concurrent calls of long-running functions with the same arguments will each execute fully and each overwrite
the memoized value as they complete. This mirrors the behavior of Python's `functools.lru_cache`. To synchronize the execution and caching
of un-cached arguments, specify `#[cached(sync_writes = true)]` / `#[once(sync_writes = true)]` (not supported by `#[io_cached]`.

- See [`cached::stores` docs](https://docs.rs/cached/latest/cached/stores/index.html) cache stores available.
- See [`proc_macro`](https://docs.rs/cached/latest/cached/proc_macro/index.html) for more procedural macro examples.
- See [`macros`](https://docs.rs/cached/latest/cached/macros/index.html) for more declarative macro examples.

**Features**

- `default`: Include `proc_macro` and `async` features
- `proc_macro`: Include proc macros
- `async`: Include support for async functions and async cache stores
- `redis_store`: Include Redis cache store
- `redis_async_std`: Include async Redis support using `async-std` and `async-std` tls support, implies `redis_store` and `async`
- `redis_tokio`: Include async Redis support using `tokio` and `tokio` tls support, implies `redis_store` and `async`
- `wasm`: Enable WASM support. Note that this feature is incompatible with all Redis features (`redis_store`, `redis_async_std`, `redis_tokio`)

The procedural macros (`#[cached]`, `#[once]`, `#[io_cached]`) offer more features, including async support.
See the [`proc_macro`](crate::proc_macro) and [`macros`](crate::macros) modules for more samples, and the
[`examples`](https://github.com/jaemk/cached/tree/master/examples) directory for runnable snippets.`

Any custom cache that implements `cached::Cached`/`cached::CachedAsync` can be used with the `#[cached]`/`#[once]`/`cached!` macros in place of the built-ins.
Any custom cache that implements `cached::IOCached`/`cached::IOCachedAsync` can be used with the `#[io_cached]` macro.

----

The basic usage looks like:

```rust,no_run
use cached::proc_macro::cached;

/// Defines a function named `fib` that uses a cache implicitly named `FIB`.
/// By default, the cache will be the function's name in all caps.
/// The following line is equivalent to #[cached(name = "FIB", unbound)]
#[cached]
fn fib(n: u64) -> u64 {
    if n == 0 || n == 1 { return n }
    fib(n-1) + fib(n-2)
}
# pub fn main() { }
```

----

```rust,no_run
use std::thread::sleep;
use std::time::Duration;
use cached::proc_macro::cached;
use cached::SizedCache;

/// Use an explicit cache-type with a custom creation block and custom cache-key generating block
#[cached(
    type = "SizedCache<String, usize>",
    create = "{ SizedCache::with_size(100) }",
    convert = r#"{ format!("{}{}", a, b) }"#
)]
fn keyed(a: &str, b: &str) -> usize {
    let size = a.len() + b.len();
    sleep(Duration::new(size as u64, 0));
    size
}
# pub fn main() { }
```

----

```rust,no_run
use cached::proc_macro::once;

/// Only cache the initial function call.
/// Function will be re-executed after the cache
/// expires (according to `time` seconds).
/// When no (or expired) cache, concurrent calls
/// will synchronize (`sync_writes`) so the function
/// is only executed once.
#[once(time=10, option = true, sync_writes = true)]
fn keyed(a: String) -> Option<usize> {
    if a == "a" {
        Some(a.len())
    } else {
        None
    }
}
# pub fn main() { }
```

----

```rust,no_run,ignore
use cached::proc_macro::io_cached;
use cached::AsyncRedisCache;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Clone)]
enum ExampleError {
    #[error("error with redis cache `{0}`")]
    RedisError(String),
}

/// Cache the results of an async function in redis. Cache
/// keys will be prefixed with `cache_redis_prefix`.
/// A `map_error` closure must be specified to convert any
/// redis cache errors into the same type of error returned
/// by your function. All `io_cached` functions must return `Result`s.
#[io_cached(
    map_error = r##"|e| ExampleError::RedisError(format!("{:?}", e))"##,
    type = "AsyncRedisCache<u64, String>",
    create = r##" {
        AsyncRedisCache::new("cached_redis_prefix", 1)
            .set_refresh(true)
            .build()
            .await
            .expect("error building example redis cache")
    } "##
)]
async fn async_cached_sleep_secs(secs: u64) -> Result<String, ExampleError> {
    std::thread::sleep(std::time::Duration::from_secs(secs));
    Ok(secs.to_string())
}
```


Functions defined via macros will have their results cached using the
function's arguments as a key, a `convert` expression specified on a procedural macros,
or a `Key` block specified on a `cached_key!` declarative macro.

When a macro-defined function is called, the function's cache is first checked for an already
computed (and still valid) value before evaluating the function body.

Due to the requirements of storing arguments and return values in a global cache:

- Function return types:
  - For all store types, except Redis, must be owned and implement `Clone`
  - For the Redis store type, must be owned and implement `serde::Serialize + serde::DeserializeOwned`
- Function arguments:
  - For all store types, except Redis, must either be owned and implement `Hash + Eq + Clone`,
    the `cached_key!` macro is used with a `Key` block specifying key construction, or
    a `convert` expression is specified on a procedural macro to specify how to construct a key
    of a `Hash + Eq + Clone` type.
  - For the Redis store type, must either be owned and implement `Display`, or the `cached_key!` & `Key`
    or procedural macro & `convert` expression used to specify how to construct a key of a `Display` type.
- Arguments and return values will be `cloned` in the process of insertion and retrieval. Except for Redis
  where arguments are formatted into `Strings` and values are de/serialized.
- Macro-defined functions should not be used to produce side-effectual results!
- Macro-defined functions cannot live directly under `impl` blocks since macros expand to a
  `once_cell`/`lazy_static` initialization and one or more function definitions.
- Macro-defined functions cannot accept `Self` types as a parameter.


*/

#[cfg(feature = "async")]
pub extern crate async_once;
#[cfg(feature = "async")]
pub extern crate lazy_static;
pub extern crate once_cell;

#[cfg(feature = "proc_macro")]
pub use proc_macro::Return;
#[cfg(any(feature = "redis_async_std", feature = "redis_tokio"))]
pub use stores::AsyncRedisCache;
pub use stores::{
    CanExpire, ExpiringValueCache, SizedCache, TimedCache, TimedSizedCache, UnboundCache,
};
#[cfg(feature = "redis_store")]
pub use stores::{RedisCache, RedisCacheError};
#[cfg(feature = "async")]
use {async_trait::async_trait, futures::Future};

mod lru_list;
pub mod macros;
#[cfg(feature = "proc_macro")]
pub mod proc_macro;
pub mod stores;
pub use instant;

#[cfg(any(feature = "proc_macro", feature = "async"))]
pub mod async_sync {
    pub use tokio::sync::Mutex;
    pub use tokio::sync::RwLock;
}

/// Cache operations
pub trait Cached<K, V> {
    /// Attempt to retrieve a cached value
    fn cache_get(&mut self, k: &K) -> Option<&V>;

    /// Attempt to retrieve a cached value with mutable access
    fn cache_get_mut(&mut self, k: &K) -> Option<&mut V>;

    /// Insert a key, value pair and return the previous value
    fn cache_set(&mut self, k: K, v: V) -> Option<V>;

    /// Get or insert a key, value pair
    fn cache_get_or_set_with<F: FnOnce() -> V>(&mut self, k: K, f: F) -> &mut V;

    /// Remove a cached value
    fn cache_remove(&mut self, k: &K) -> Option<V>;

    /// Remove all cached values. Keeps the allocated memory for reuse.
    fn cache_clear(&mut self);

    /// Remove all cached values. Free memory and return to initial state
    fn cache_reset(&mut self);

    /// Reset misses/hits counters
    fn cache_reset_metrics(&mut self) {}

    /// Return the current cache size (number of elements)
    fn cache_size(&self) -> usize;

    /// Return the number of times a cached value was successfully retrieved
    fn cache_hits(&self) -> Option<u64> {
        None
    }

    /// Return the number of times a cached value was unable to be retrieved
    fn cache_misses(&self) -> Option<u64> {
        None
    }

    /// Return the cache capacity
    fn cache_capacity(&self) -> Option<usize> {
        None
    }

    /// Return the lifespan of cached values (time to eviction)
    fn cache_lifespan(&self) -> Option<u64> {
        None
    }

    /// Set the lifespan of cached values, returns the old value
    fn cache_set_lifespan(&mut self, _seconds: u64) -> Option<u64> {
        None
    }
}

#[cfg(feature = "async")]
#[async_trait]
pub trait CachedAsync<K, V> {
    async fn get_or_set_with<F, Fut>(&mut self, k: K, f: F) -> &mut V
    where
        V: Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = V> + Send;

    async fn try_get_or_set_with<F, Fut, E>(&mut self, k: K, f: F) -> Result<&mut V, E>
    where
        V: Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<V, E>> + Send;
}

/// Cache operations on an io-connected store
pub trait IOCached<K, V> {
    type Error;

    /// Attempt to retrieve a cached value
    fn cache_get(&self, k: &K) -> Result<Option<V>, Self::Error>;

    /// Insert a key, value pair and return the previous value
    fn cache_set(&self, k: K, v: V) -> Result<Option<V>, Self::Error>;

    /// Remove a cached value
    fn cache_remove(&self, k: &K) -> Result<Option<V>, Self::Error>;

    /// Set the flag to control whether cache hits refresh the ttl of cached values, returns the old flag value
    fn cache_set_refresh(&mut self, refresh: bool) -> bool;

    /// Return the lifespan of cached values (time to eviction)
    fn cache_lifespan(&self) -> Option<u64> {
        None
    }

    /// Set the lifespan of cached values, returns the old value
    fn cache_set_lifespan(&mut self, _seconds: u64) -> Option<u64> {
        None
    }
}

#[cfg(feature = "async")]
#[async_trait]
pub trait IOCachedAsync<K, V> {
    type Error;
    async fn cache_get(&self, k: &K) -> Result<Option<V>, Self::Error>;

    async fn cache_set(&self, k: K, v: V) -> Result<Option<V>, Self::Error>;

    /// Remove a cached value
    async fn cache_remove(&self, k: &K) -> Result<Option<V>, Self::Error>;

    /// Set the flag to control whether cache hits refresh the ttl of cached values, returns the old flag value
    fn cache_set_refresh(&mut self, refresh: bool) -> bool;

    /// Return the lifespan of cached values (time to eviction)
    fn cache_lifespan(&self) -> Option<u64> {
        None
    }

    /// Set the lifespan of cached values, returns the old value
    fn cache_set_lifespan(&mut self, _seconds: u64) -> Option<u64> {
        None
    }
}
