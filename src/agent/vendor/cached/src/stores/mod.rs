use crate::Cached;
use std::cmp::Eq;
#[cfg(feature = "async")]
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(feature = "async")]
use {super::CachedAsync, async_trait::async_trait, futures::Future};

mod expiring_value_cache;
#[cfg(feature = "redis_store")]
mod redis;
mod sized;
mod timed;
mod timed_sized;
mod unbound;

#[cfg(feature = "redis_store")]
pub use crate::stores::redis::{
    RedisCache, RedisCacheBuildError, RedisCacheBuilder, RedisCacheError,
};
pub use expiring_value_cache::{CanExpire, ExpiringValueCache};
pub use sized::SizedCache;
pub use timed::TimedCache;
pub use timed_sized::TimedSizedCache;
pub use unbound::UnboundCache;

#[cfg(all(
    feature = "async",
    feature = "redis_store",
    any(feature = "redis_async_std", feature = "redis_tokio")
))]
pub use crate::stores::redis::{AsyncRedisCache, AsyncRedisCacheBuilder};

impl<K: Hash + Eq, V> Cached<K, V> for HashMap<K, V> {
    fn cache_get(&mut self, k: &K) -> Option<&V> {
        self.get(k)
    }
    fn cache_get_mut(&mut self, k: &K) -> Option<&mut V> {
        self.get_mut(k)
    }
    fn cache_get_or_set_with<F: FnOnce() -> V>(&mut self, key: K, f: F) -> &mut V {
        self.entry(key).or_insert_with(f)
    }
    fn cache_set(&mut self, k: K, v: V) -> Option<V> {
        self.insert(k, v)
    }
    fn cache_remove(&mut self, k: &K) -> Option<V> {
        self.remove(k)
    }
    fn cache_clear(&mut self) {
        self.clear();
    }
    fn cache_reset(&mut self) {
        *self = HashMap::new();
    }
    fn cache_size(&self) -> usize {
        self.len()
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl<K, V> CachedAsync<K, V> for HashMap<K, V>
where
    K: Hash + Eq + Clone + Send,
{
    async fn get_or_set_with<F, Fut>(&mut self, k: K, f: F) -> &mut V
    where
        V: Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = V> + Send,
    {
        match self.entry(k) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(f().await),
        }
    }

    async fn try_get_or_set_with<F, Fut, E>(&mut self, k: K, f: F) -> Result<&mut V, E>
    where
        V: Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<V, E>> + Send,
    {
        let v = match self.entry(k) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(f().await?),
        };

        Ok(v)
    }
}

#[cfg(test)]
/// Cache store tests
mod tests {
    use super::*;

    #[test]
    fn hashmap() {
        let mut c = std::collections::HashMap::new();
        assert!(c.cache_get(&1).is_none());
        assert_eq!(c.cache_misses(), None);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_get(&1), Some(&100));
        assert_eq!(c.cache_hits(), None);
        assert_eq!(c.cache_misses(), None);
    }
}
