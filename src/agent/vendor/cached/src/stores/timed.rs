use std::cmp::Eq;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::Hash;

use instant::Instant;

#[cfg(feature = "async")]
use {super::CachedAsync, async_trait::async_trait, futures::Future};

use super::Cached;

/// Enum used for defining the status of time-cached values
#[derive(Debug)]
pub(super) enum Status {
    NotFound,
    Found,
    Expired,
}

/// Cache store bound by time
///
/// Values are timestamped when inserted and are
/// evicted if expired at time of retrieval.
///
/// Note: This cache is in-memory only
#[derive(Clone, Debug)]
pub struct TimedCache<K, V> {
    pub(super) store: HashMap<K, (Instant, V)>,
    pub(super) seconds: u64,
    pub(super) hits: u64,
    pub(super) misses: u64,
    pub(super) initial_capacity: Option<usize>,
    pub(super) refresh: bool,
}

impl<K: Hash + Eq, V> TimedCache<K, V> {
    /// Creates a new `TimedCache` with a specified lifespan
    pub fn with_lifespan(seconds: u64) -> TimedCache<K, V> {
        Self::with_lifespan_and_refresh(seconds, false)
    }

    /// Creates a new `TimedCache` with a specified lifespan and
    /// cache-store with the specified pre-allocated capacity
    pub fn with_lifespan_and_capacity(seconds: u64, size: usize) -> TimedCache<K, V> {
        TimedCache {
            store: Self::new_store(Some(size)),
            seconds,
            hits: 0,
            misses: 0,
            initial_capacity: Some(size),
            refresh: false,
        }
    }

    /// Creates a new `TimedCache` with a specified lifespan which
    /// refreshes the ttl when the entry is retrieved
    pub fn with_lifespan_and_refresh(seconds: u64, refresh: bool) -> TimedCache<K, V> {
        TimedCache {
            store: Self::new_store(None),
            seconds,
            hits: 0,
            misses: 0,
            initial_capacity: None,
            refresh,
        }
    }

    /// Returns if the lifetime is refreshed when the value is retrieved
    pub fn refresh(&self) -> bool {
        self.refresh
    }

    /// Sets if the lifetime is refreshed when the value is retrieved
    pub fn set_refresh(&mut self, refresh: bool) {
        self.refresh = refresh
    }

    fn new_store(capacity: Option<usize>) -> HashMap<K, (Instant, V)> {
        capacity.map_or_else(HashMap::new, HashMap::with_capacity)
    }

    /// Returns a reference to the cache's `store`
    pub fn get_store(&self) -> &HashMap<K, (Instant, V)> {
        &self.store
    }

    /// Remove any expired values from the cache
    pub fn flush(&mut self) {
        let seconds = self.seconds;
        self.store
            .retain(|_, (instant, _)| instant.elapsed().as_secs() < seconds);
    }
}

impl<K: Hash + Eq, V> Cached<K, V> for TimedCache<K, V> {
    fn cache_get(&mut self, key: &K) -> Option<&V> {
        let status = {
            let mut val = self.store.get_mut(key);
            if let Some(&mut (instant, _)) = val.as_mut() {
                if instant.elapsed().as_secs() < self.seconds {
                    if self.refresh {
                        *instant = Instant::now();
                    }
                    Status::Found
                } else {
                    Status::Expired
                }
            } else {
                Status::NotFound
            }
        };
        match status {
            Status::NotFound => {
                self.misses += 1;
                None
            }
            Status::Found => {
                self.hits += 1;
                self.store.get(key).map(|stamped| &stamped.1)
            }
            Status::Expired => {
                self.misses += 1;
                self.store.remove(key).unwrap();
                None
            }
        }
    }

    fn cache_get_mut(&mut self, key: &K) -> Option<&mut V> {
        let status = {
            let mut val = self.store.get_mut(key);
            if let Some(&mut (instant, _)) = val.as_mut() {
                if instant.elapsed().as_secs() < self.seconds {
                    if self.refresh {
                        *instant = Instant::now();
                    }
                    Status::Found
                } else {
                    Status::Expired
                }
            } else {
                Status::NotFound
            }
        };
        match status {
            Status::NotFound => {
                self.misses += 1;
                None
            }
            Status::Found => {
                self.hits += 1;
                self.store.get_mut(key).map(|stamped| &mut stamped.1)
            }
            Status::Expired => {
                self.misses += 1;
                self.store.remove(key).unwrap();
                None
            }
        }
    }

    fn cache_get_or_set_with<F: FnOnce() -> V>(&mut self, key: K, f: F) -> &mut V {
        match self.store.entry(key) {
            Entry::Occupied(mut occupied) => {
                if occupied.get().0.elapsed().as_secs() < self.seconds {
                    if self.refresh {
                        occupied.get_mut().0 = Instant::now();
                    }
                    self.hits += 1;
                } else {
                    self.misses += 1;
                    let val = f();
                    occupied.insert((Instant::now(), val));
                }
                &mut occupied.into_mut().1
            }
            Entry::Vacant(vacant) => {
                self.misses += 1;
                let val = f();
                &mut vacant.insert((Instant::now(), val)).1
            }
        }
    }

    fn cache_set(&mut self, key: K, val: V) -> Option<V> {
        let stamped = (Instant::now(), val);
        self.store.insert(key, stamped).and_then(|(instant, v)| {
            if instant.elapsed().as_secs() < self.seconds {
                Some(v)
            } else {
                None
            }
        })
    }
    fn cache_remove(&mut self, k: &K) -> Option<V> {
        self.store.remove(k).and_then(|(instant, v)| {
            if instant.elapsed().as_secs() < self.seconds {
                Some(v)
            } else {
                None
            }
        })
    }
    fn cache_clear(&mut self) {
        self.store.clear();
    }
    fn cache_reset_metrics(&mut self) {
        self.misses = 0;
        self.hits = 0;
    }
    fn cache_reset(&mut self) {
        self.store = Self::new_store(self.initial_capacity);
    }
    fn cache_size(&self) -> usize {
        self.store.len()
    }
    fn cache_hits(&self) -> Option<u64> {
        Some(self.hits)
    }
    fn cache_misses(&self) -> Option<u64> {
        Some(self.misses)
    }
    fn cache_lifespan(&self) -> Option<u64> {
        Some(self.seconds)
    }

    fn cache_set_lifespan(&mut self, seconds: u64) -> Option<u64> {
        let old = self.seconds;
        self.seconds = seconds;
        Some(old)
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl<K, V> CachedAsync<K, V> for TimedCache<K, V>
where
    K: Hash + Eq + Clone + Send,
{
    async fn get_or_set_with<F, Fut>(&mut self, k: K, f: F) -> &mut V
    where
        V: Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = V> + Send,
    {
        match self.store.entry(k) {
            Entry::Occupied(mut occupied) => {
                if occupied.get().0.elapsed().as_secs() < self.seconds {
                    if self.refresh {
                        occupied.get_mut().0 = Instant::now();
                    }
                    self.hits += 1;
                } else {
                    self.misses += 1;
                    occupied.insert((Instant::now(), f().await));
                }
                &mut occupied.into_mut().1
            }
            Entry::Vacant(vacant) => {
                self.misses += 1;
                &mut vacant.insert((Instant::now(), f().await)).1
            }
        }
    }

    async fn try_get_or_set_with<F, Fut, E>(&mut self, k: K, f: F) -> Result<&mut V, E>
    where
        V: Send,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<V, E>> + Send,
    {
        let v = match self.store.entry(k) {
            Entry::Occupied(mut occupied) => {
                if occupied.get().0.elapsed().as_secs() < self.seconds {
                    if self.refresh {
                        occupied.get_mut().0 = Instant::now();
                    }
                    self.hits += 1;
                } else {
                    self.misses += 1;
                    occupied.insert((Instant::now(), f().await?));
                }
                &mut occupied.into_mut().1
            }
            Entry::Vacant(vacant) => {
                self.misses += 1;
                &mut vacant.insert((Instant::now(), f().await?)).1
            }
        };

        Ok(v)
    }
}

#[cfg(test)]
/// Cache store tests
mod tests {
    use std::{thread::sleep, time::Duration};

    use super::*;

    #[test]
    fn timed_cache() {
        let mut c = TimedCache::with_lifespan(2);
        assert!(c.cache_get(&1).is_none());
        let misses = c.cache_misses().unwrap();
        assert_eq!(1, misses);

        assert_eq!(c.cache_set(1, 100), None);
        assert!(c.cache_get(&1).is_some());
        let hits = c.cache_hits().unwrap();
        let misses = c.cache_misses().unwrap();
        assert_eq!(1, hits);
        assert_eq!(1, misses);

        sleep(Duration::new(2, 0));
        assert!(c.cache_get(&1).is_none());
        let misses = c.cache_misses().unwrap();
        assert_eq!(2, misses);

        let old = c.cache_set_lifespan(1).unwrap();
        assert_eq!(2, old);
        assert_eq!(c.cache_set(1, 100), None);
        assert!(c.cache_get(&1).is_some());
        let hits = c.cache_hits().unwrap();
        let misses = c.cache_misses().unwrap();
        assert_eq!(2, hits);
        assert_eq!(2, misses);

        sleep(Duration::new(1, 0));
        assert!(c.cache_get(&1).is_none());
        let misses = c.cache_misses().unwrap();
        assert_eq!(3, misses);
    }

    #[test]
    fn timed_cache_refresh() {
        let mut c = TimedCache::with_lifespan_and_refresh(2, true);
        assert!(c.refresh());
        assert_eq!(c.cache_get(&1), None);
        let misses = c.cache_misses().unwrap();
        assert_eq!(1, misses);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_get(&1), Some(&100));
        let hits = c.cache_hits().unwrap();
        let misses = c.cache_misses().unwrap();
        assert_eq!(1, hits);
        assert_eq!(1, misses);

        assert_eq!(c.cache_set(2, 200), None);
        assert_eq!(c.cache_get(&2), Some(&200));
        sleep(Duration::new(1, 0));
        assert_eq!(c.cache_get(&1), Some(&100));
        sleep(Duration::new(1, 0));
        assert_eq!(c.cache_get(&1), Some(&100));
        assert_eq!(c.cache_get(&2), None);
    }

    #[test]
    fn clear() {
        let mut c = TimedCache::with_lifespan(3600);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(2, 200), None);
        assert_eq!(c.cache_set(3, 300), None);
        c.cache_clear();

        assert_eq!(0, c.cache_size());
    }

    #[test]
    fn reset() {
        let mut c = TimedCache::with_lifespan(100);
        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(2, 200), None);
        assert_eq!(c.cache_set(3, 300), None);
        assert!(3 <= c.store.capacity());

        c.cache_reset();

        assert_eq!(0, c.store.capacity());

        let init_capacity = 1;
        let mut c = TimedCache::with_lifespan_and_capacity(100, init_capacity);
        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(2, 200), None);
        assert_eq!(c.cache_set(3, 300), None);
        assert!(3 <= c.store.capacity());

        c.cache_reset();

        assert!(init_capacity <= c.store.capacity());
    }

    #[test]
    fn remove() {
        let mut c = TimedCache::with_lifespan(3600);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(2, 200), None);
        assert_eq!(c.cache_set(3, 300), None);

        assert_eq!(Some(100), c.cache_remove(&1));
        assert_eq!(2, c.cache_size());
    }

    #[test]
    fn remove_expired() {
        let mut c = TimedCache::with_lifespan(1);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(1, 200), Some(100));
        assert_eq!(c.cache_size(), 1);

        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(None, c.cache_remove(&1));
        assert_eq!(0, c.cache_size());
    }

    #[test]
    fn insert_expired() {
        let mut c = TimedCache::with_lifespan(1);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(1, 200), Some(100));
        assert_eq!(c.cache_size(), 1);

        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(1, c.cache_size());
        assert_eq!(None, c.cache_set(1, 300));
        assert_eq!(1, c.cache_size());
    }

    #[test]
    fn get_expired() {
        let mut c = TimedCache::with_lifespan(1);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(1, 200), Some(100));
        assert_eq!(c.cache_size(), 1);

        std::thread::sleep(std::time::Duration::from_secs(1));
        // still around until we try to get
        assert_eq!(1, c.cache_size());
        assert_eq!(None, c.cache_get(&1));
        assert_eq!(0, c.cache_size());
    }

    #[test]
    fn get_mut_expired() {
        let mut c = TimedCache::with_lifespan(1);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(1, 200), Some(100));
        assert_eq!(c.cache_size(), 1);

        std::thread::sleep(std::time::Duration::from_secs(1));
        // still around until we try to get
        assert_eq!(1, c.cache_size());
        assert_eq!(None, c.cache_get_mut(&1));
        assert_eq!(0, c.cache_size());
    }

    #[test]
    fn flush_expired() {
        let mut c = TimedCache::with_lifespan(1);

        assert_eq!(c.cache_set(1, 100), None);
        assert_eq!(c.cache_set(1, 200), Some(100));
        assert_eq!(c.cache_size(), 1);

        std::thread::sleep(std::time::Duration::from_secs(1));
        // still around until we flush
        assert_eq!(1, c.cache_size());
        c.flush();
        assert_eq!(0, c.cache_size());
    }

    #[test]
    fn get_or_set_with() {
        let mut c = TimedCache::with_lifespan(2);

        assert_eq!(c.cache_get_or_set_with(0, || 0), &0);
        assert_eq!(c.cache_get_or_set_with(1, || 1), &1);
        assert_eq!(c.cache_get_or_set_with(2, || 2), &2);
        assert_eq!(c.cache_get_or_set_with(3, || 3), &3);
        assert_eq!(c.cache_get_or_set_with(4, || 4), &4);
        assert_eq!(c.cache_get_or_set_with(5, || 5), &5);

        assert_eq!(c.cache_misses(), Some(6));

        assert_eq!(c.cache_get_or_set_with(0, || 0), &0);

        assert_eq!(c.cache_misses(), Some(6));

        assert_eq!(c.cache_get_or_set_with(0, || 42), &0);

        assert_eq!(c.cache_misses(), Some(6));

        sleep(Duration::new(2, 0));

        assert_eq!(c.cache_get_or_set_with(1, || 42), &42);

        assert_eq!(c.cache_misses(), Some(7));
    }
}
