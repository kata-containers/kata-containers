use super::{Cached, SizedCache};
use crate::stores::timed::Status;
use std::hash::Hash;

/// The CanExpire trait defines a function for implementations to determine if
/// the value has expired.
pub trait CanExpire {
    /// is_expired returns whether the value has expired.
    fn is_expired(&self) -> bool;
}

/// Expiring Value Cache
///
/// Stores values that implement the CanExpire trait so that expiration
/// is determined by the values themselves. This is useful for caching
/// values which themselves contain an expiry timestamp.
///
/// Note: This cache is in-memory only.
#[derive(Clone, Debug)]
pub struct ExpiringValueCache<K: Hash + Eq, V: CanExpire> {
    pub(super) store: SizedCache<K, V>,
    pub(super) hits: u64,
    pub(super) misses: u64,
}

impl<K: Clone + Hash + Eq, V: CanExpire> ExpiringValueCache<K, V> {
    /// Creates a new `ExpiringValueCache` with a given size limit and
    /// pre-allocated backing data.
    pub fn with_size(size: usize) -> ExpiringValueCache<K, V> {
        ExpiringValueCache {
            store: SizedCache::with_size(size),
            hits: 0,
            misses: 0,
        }
    }

    fn status(&mut self, k: &K) -> Status {
        let v = self.store.cache_get(k);
        match v {
            Some(v) => match v.is_expired() {
                true => Status::Expired,
                false => Status::Found,
            },
            None => Status::NotFound,
        }
    }
}

// https://docs.rs/cached/latest/cached/trait.Cached.html
impl<K: Hash + Eq + Clone, V: CanExpire> Cached<K, V> for ExpiringValueCache<K, V> {
    fn cache_get(&mut self, k: &K) -> Option<&V> {
        match self.status(k) {
            Status::NotFound => {
                self.misses += 1;
                None
            }
            Status::Found => {
                self.hits += 1;
                self.store.cache_get(k)
            }
            Status::Expired => {
                self.misses += 1;
                self.store.cache_remove(k);
                None
            }
        }
    }

    fn cache_get_mut(&mut self, k: &K) -> Option<&mut V> {
        match self.status(k) {
            Status::NotFound => {
                self.misses += 1;
                None
            }
            Status::Found => {
                self.hits += 1;
                self.store.cache_get_mut(k)
            }
            Status::Expired => {
                self.misses += 1;
                self.store.cache_remove(k);
                None
            }
        }
    }

    fn cache_get_or_set_with<F: FnOnce() -> V>(&mut self, k: K, f: F) -> &mut V {
        // get_or_set_with_if will set the value in the cache if an existing
        // value is not valid, which, in our case, is if the value has expired.
        let (was_present, was_valid, v) = self.store.get_or_set_with_if(k, f, |v| !v.is_expired());
        if was_present && was_valid {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        v
    }
    fn cache_set(&mut self, k: K, v: V) -> Option<V> {
        self.store.cache_set(k, v)
    }
    fn cache_remove(&mut self, k: &K) -> Option<V> {
        self.store.cache_remove(k)
    }
    fn cache_clear(&mut self) {
        self.store.cache_clear();
    }
    fn cache_reset(&mut self) {
        self.store.cache_reset()
    }
    fn cache_size(&self) -> usize {
        self.store.cache_size()
    }
    fn cache_hits(&self) -> Option<u64> {
        Some(self.hits)
    }
    fn cache_misses(&self) -> Option<u64> {
        Some(self.misses)
    }
    fn cache_reset_metrics(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }
}

#[cfg(test)]
/// Expiring Value Cache tests
mod tests {
    use super::*;

    type ExpiredU8 = u8;

    impl CanExpire for ExpiredU8 {
        fn is_expired(&self) -> bool {
            *self > 10
        }
    }

    #[test]
    fn expiring_value_cache_get_miss() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);

        // Getting a non-existent cache key.
        assert!(c.cache_get(&1).is_none());
        assert_eq!(c.cache_hits(), Some(0));
        assert_eq!(c.cache_misses(), Some(1));
    }

    #[test]
    fn expiring_value_cache_get_hit() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);

        // Getting a cached value.
        assert!(c.cache_set(1, 2).is_none());
        assert_eq!(c.cache_get(&1), Some(&2));
        assert_eq!(c.cache_hits(), Some(1));
        assert_eq!(c.cache_misses(), Some(0));
    }

    #[test]
    fn expiring_value_cache_get_expired() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);

        assert!(c.cache_set(2, 12).is_none());

        assert!(c.cache_get(&2).is_none());
        assert_eq!(c.cache_hits(), Some(0));
        assert_eq!(c.cache_misses(), Some(1));
    }

    #[test]
    fn expiring_value_cache_get_mut_miss() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);

        // Getting a non-existent cache key.
        assert!(c.cache_get_mut(&1).is_none());
        assert_eq!(c.cache_hits(), Some(0));
        assert_eq!(c.cache_misses(), Some(1));
    }

    #[test]
    fn expiring_value_cache_get_mut_hit() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);

        // Getting a cached value.
        assert!(c.cache_set(1, 2).is_none());
        assert_eq!(c.cache_get_mut(&1), Some(&mut 2));
        assert_eq!(c.cache_hits(), Some(1));
        assert_eq!(c.cache_misses(), Some(0));
    }

    #[test]
    fn expiring_value_cache_get_mut_expired() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);

        assert!(c.cache_set(2, 12).is_none());

        assert!(c.cache_get(&2).is_none());
        assert_eq!(c.cache_hits(), Some(0));
        assert_eq!(c.cache_misses(), Some(1));
    }

    #[test]
    fn expiring_value_cache_get_or_set_with_missing() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);

        assert_eq!(c.cache_get_or_set_with(1, || 1), &1);
        assert_eq!(c.cache_hits(), Some(0));
        assert_eq!(c.cache_misses(), Some(1));
    }

    #[test]
    fn expiring_value_cache_get_or_set_with_present() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);
        assert!(c.cache_set(1, 5).is_none());

        // Existing value is returned rather than setting new value.
        assert_eq!(c.cache_get_or_set_with(1, || 1), &5);
        assert_eq!(c.cache_hits(), Some(1));
        assert_eq!(c.cache_misses(), Some(0));
    }

    #[test]
    fn expiring_value_cache_get_or_set_with_expired() {
        let mut c: ExpiringValueCache<u8, ExpiredU8> = ExpiringValueCache::with_size(3);
        assert!(c.cache_set(1, 11).is_none());

        // New value is returned as existing had expired.
        assert_eq!(c.cache_get_or_set_with(1, || 1), &1);
        assert_eq!(c.cache_hits(), Some(0));
        assert_eq!(c.cache_misses(), Some(1));
    }
}
