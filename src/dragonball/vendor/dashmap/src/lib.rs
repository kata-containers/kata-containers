#![allow(clippy::type_complexity)]

pub mod iter;
pub mod iter_set;
pub mod lock;
pub mod mapref;
mod read_only;
#[cfg(feature = "serde")]
mod serde;
mod set;
pub mod setref;
mod t;
mod util;

#[cfg(feature = "rayon")]
pub mod rayon {
    pub mod map;
    pub mod set;
}

use cfg_if::cfg_if;
use core::borrow::Borrow;
use core::fmt;
use core::hash::{BuildHasher, Hash, Hasher};
use core::iter::FromIterator;
use core::ops::{BitAnd, BitOr, Shl, Shr, Sub};
use iter::{Iter, IterMut, OwningIter};
use lock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use mapref::entry::{Entry, OccupiedEntry, VacantEntry};
use mapref::multiple::RefMulti;
use mapref::one::{Ref, RefMut};
pub use read_only::ReadOnlyView;
pub use set::DashSet;
use std::collections::hash_map::RandomState;
pub use t::Map;

cfg_if! {
    if #[cfg(feature = "raw-api")] {
        pub use util::SharedValue;
    } else {
        use util::SharedValue;
    }
}

pub(crate) type HashMap<K, V, S> = std::collections::HashMap<K, SharedValue<V>, S>;

fn shard_amount() -> usize {
    (num_cpus::get() * 4).next_power_of_two()
}

fn ncb(shard_amount: usize) -> usize {
    shard_amount.trailing_zeros() as usize
}

/// DashMap is an implementation of a concurrent associative array/hashmap in Rust.
///
/// DashMap tries to implement an easy to use API similar to `std::collections::HashMap`
/// with some slight changes to handle concurrency.
///
/// DashMap tries to be very simple to use and to be a direct replacement for `RwLock<HashMap<K, V, S>>`.
/// To accomplish these all methods take `&self` instead modifying methods taking `&mut self`.
/// This allows you to put a DashMap in an `Arc<T>` and share it between threads while being able to modify it.
///
/// Documentation mentioning locking behaviour acts in the reference frame of the calling thread.
/// This means that it is safe to ignore it across multiple threads.
pub struct DashMap<K, V, S = RandomState> {
    shift: usize,
    shards: Box<[RwLock<HashMap<K, V, S>>]>,
    hasher: S,
}

impl<K: Eq + Hash + Clone, V: Clone, S: Clone> Clone for DashMap<K, V, S> {
    fn clone(&self) -> Self {
        let mut inner_shards = Vec::new();

        for shard in self.shards.iter() {
            let shard = shard.read();

            inner_shards.push(RwLock::new((*shard).clone()));
        }

        Self {
            shift: self.shift,
            shards: inner_shards.into_boxed_slice(),
            hasher: self.hasher.clone(),
        }
    }
}

impl<K, V, S> Default for DashMap<K, V, S>
where
    K: Eq + Hash,
    S: Default + BuildHasher + Clone,
{
    fn default() -> Self {
        Self::with_hasher(Default::default())
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a> DashMap<K, V, RandomState> {
    /// Creates a new DashMap with a capacity of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let reviews = DashMap::new();
    /// reviews.insert("Veloren", "What a fantastic game!");
    /// ```
    pub fn new() -> Self {
        DashMap::with_hasher(RandomState::default())
    }

    /// Creates a new DashMap with a specified starting capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let mappings = DashMap::with_capacity(2);
    /// mappings.insert(2, 4);
    /// mappings.insert(8, 16);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        DashMap::with_capacity_and_hasher(capacity, RandomState::default())
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a, S: BuildHasher + Clone> DashMap<K, V, S> {
    /// Wraps this `DashMap` into a read-only view. This view allows to obtain raw references to the stored values.
    pub fn into_read_only(self) -> ReadOnlyView<K, V, S> {
        ReadOnlyView::new(self)
    }

    /// Creates a new DashMap with a capacity of 0 and the provided hasher.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// let reviews = DashMap::with_hasher(s);
    /// reviews.insert("Veloren", "What a fantastic game!");
    /// ```
    pub fn with_hasher(hasher: S) -> Self {
        Self::with_capacity_and_hasher(0, hasher)
    }

    /// Creates a new DashMap with a specified starting capacity and hasher.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mappings = DashMap::with_capacity_and_hasher(2, s);
    /// mappings.insert(2, 4);
    /// mappings.insert(8, 16);
    /// ```
    pub fn with_capacity_and_hasher(mut capacity: usize, hasher: S) -> Self {
        let shard_amount = shard_amount();
        let shift = util::ptr_size_bits() - ncb(shard_amount);

        if capacity != 0 {
            capacity = (capacity + (shard_amount - 1)) & !(shard_amount - 1);
        }

        let cps = capacity / shard_amount;

        let shards = (0..shard_amount)
            .map(|_| RwLock::new(HashMap::with_capacity_and_hasher(cps, hasher.clone())))
            .collect();

        Self {
            shift,
            shards,
            hasher,
        }
    }

    /// Hash a given item to produce a usize.
    /// Uses the provided or default HashBuilder.
    pub fn hash_usize<T: Hash>(&self, item: &T) -> usize {
        let mut hasher = self.hasher.build_hasher();

        item.hash(&mut hasher);

        hasher.finish() as usize
    }

    cfg_if! {
        if #[cfg(feature = "raw-api")] {
            /// Allows you to peek at the inner shards that store your data.
            /// You should probably not use this unless you know what you are doing.
            ///
            /// Requires the `raw-api` feature to be enabled.
            ///
            /// # Examples
            ///
            /// ```
            /// use dashmap::DashMap;
            ///
            /// let map = DashMap::<(), ()>::new();
            /// println!("Amount of shards: {}", map.shards().len());
            /// ```
            pub fn shards(&self) -> &[RwLock<HashMap<K, V, S>>] {
                &self.shards
            }
        } else {
            #[allow(dead_code)]
            pub(crate) fn shards(&self) -> &[RwLock<HashMap<K, V, S>>] {
                &self.shards
            }
        }
    }

    cfg_if! {
        if #[cfg(feature = "raw-api")] {
            /// Finds which shard a certain key is stored in.
            /// You should probably not use this unless you know what you are doing.
            /// Note that shard selection is dependent on the default or provided HashBuilder.
            ///
            /// Requires the `raw-api` feature to be enabled.
            ///
            /// # Examples
            ///
            /// ```
            /// use dashmap::DashMap;
            ///
            /// let map = DashMap::new();
            /// map.insert("coca-cola", 1.4);
            /// println!("coca-cola is stored in shard: {}", map.determine_map("coca-cola"));
            /// ```
            pub fn determine_map<Q>(&self, key: &Q) -> usize
            where
                K: Borrow<Q>,
                Q: Hash + Eq + ?Sized,
            {
                let hash = self.hash_usize(&key);
                self.determine_shard(hash)
            }
        }
    }

    cfg_if! {
        if #[cfg(feature = "raw-api")] {
            /// Finds which shard a certain hash is stored in.
            ///
            /// Requires the `raw-api` feature to be enabled.
            ///
            /// # Examples
            ///
            /// ```
            /// use dashmap::DashMap;
            ///
            /// let map: DashMap<i32, i32> = DashMap::new();
            /// let key = "key";
            /// let hash = map.hash_usize(&key);
            /// println!("hash is stored in shard: {}", map.determine_shard(hash));
            /// ```
            pub fn determine_shard(&self, hash: usize) -> usize {
                // Leave the high 7 bits for the HashBrown SIMD tag.
                (hash << 7) >> self.shift
            }
        } else {

            pub(crate) fn determine_shard(&self, hash: usize) -> usize {
                // Leave the high 7 bits for the HashBrown SIMD tag.
                (hash << 7) >> self.shift
            }
        }
    }

    /// Returns a reference to the map's [`BuildHasher`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashmap::DashMap;
    /// use std::collections::hash_map::RandomState;
    ///
    /// let hasher = RandomState::new();
    /// let map: DashMap<i32, i32> = DashMap::new();
    /// let hasher: &RandomState = map.hasher();
    /// ```
    ///
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    pub fn hasher(&self) -> &S {
        &self.hasher
    }

    /// Inserts a key and a value into the map. Returns the old value associated with the key if there was one.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let map = DashMap::new();
    /// map.insert("I am the key!", "And I am the value!");
    /// ```
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self._insert(key, value)
    }

    /// Removes an entry from the map, returning the key and value if they existed in the map.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let soccer_team = DashMap::new();
    /// soccer_team.insert("Jack", "Goalie");
    /// assert_eq!(soccer_team.remove("Jack").unwrap().1, "Goalie");
    /// ```
    pub fn remove<Q>(&self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self._remove(key)
    }

    /// Removes an entry from the map, returning the key and value
    /// if the entry existed and the provided conditional function returned true.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let soccer_team = DashMap::new();
    /// soccer_team.insert("Sam", "Forward");
    /// soccer_team.remove_if("Sam", |_, position| position == &"Goalie");
    /// assert!(soccer_team.contains_key("Sam"));
    /// ```
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let soccer_team = DashMap::new();
    /// soccer_team.insert("Sam", "Forward");
    /// soccer_team.remove_if("Sam", |_, position| position == &"Forward");
    /// assert!(!soccer_team.contains_key("Sam"));
    /// ```
    pub fn remove_if<Q>(&self, key: &Q, f: impl FnOnce(&K, &V) -> bool) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self._remove_if(key, f)
    }

    /// Creates an iterator over a DashMap yielding immutable references.
    ///
    /// **Locking behaviour:** May deadlock if called when holding a mutable reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let words = DashMap::new();
    /// words.insert("hello", "world");
    /// assert_eq!(words.iter().count(), 1);
    /// ```
    pub fn iter(&'a self) -> Iter<'a, K, V, S, DashMap<K, V, S>> {
        self._iter()
    }

    /// Iterator over a DashMap yielding mutable references.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let map = DashMap::new();
    /// map.insert("Johnny", 21);
    /// map.iter_mut().for_each(|mut r| *r += 1);
    /// assert_eq!(*map.get("Johnny").unwrap(), 22);
    /// ```
    pub fn iter_mut(&'a self) -> IterMut<'a, K, V, S, DashMap<K, V, S>> {
        self._iter_mut()
    }

    /// Get a immutable reference to an entry in the map
    ///
    /// **Locking behaviour:** May deadlock if called when holding a mutable reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let youtubers = DashMap::new();
    /// youtubers.insert("Bosnian Bill", 457000);
    /// assert_eq!(*youtubers.get("Bosnian Bill").unwrap(), 457000);
    /// ```
    pub fn get<Q>(&'a self, key: &Q) -> Option<Ref<'a, K, V, S>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self._get(key)
    }

    /// Get a mutable reference to an entry in the map
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let class = DashMap::new();
    /// class.insert("Albin", 15);
    /// *class.get_mut("Albin").unwrap() -= 1;
    /// assert_eq!(*class.get("Albin").unwrap(), 14);
    /// ```
    pub fn get_mut<Q>(&'a self, key: &Q) -> Option<RefMut<'a, K, V, S>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self._get_mut(key)
    }

    /// Remove excess capacity to reduce memory usage.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    pub fn shrink_to_fit(&self) {
        self._shrink_to_fit();
    }

    /// Retain elements that whose predicates return true
    /// and discard elements whose predicates return false.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let people = DashMap::new();
    /// people.insert("Albin", 15);
    /// people.insert("Jones", 22);
    /// people.insert("Charlie", 27);
    /// people.retain(|_, v| *v > 20);
    /// assert_eq!(people.len(), 2);
    /// ```
    pub fn retain(&self, f: impl FnMut(&K, &mut V) -> bool) {
        self._retain(f);
    }

    /// Fetches the total number of key-value pairs stored in the map.
    ///
    /// **Locking behaviour:** May deadlock if called when holding a mutable reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let people = DashMap::new();
    /// people.insert("Albin", 15);
    /// people.insert("Jones", 22);
    /// people.insert("Charlie", 27);
    /// assert_eq!(people.len(), 3);
    /// ```
    pub fn len(&self) -> usize {
        self._len()
    }

    /// Checks if the map is empty or not.
    ///
    /// **Locking behaviour:** May deadlock if called when holding a mutable reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let map = DashMap::<(), ()>::new();
    /// assert!(map.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self._is_empty()
    }

    /// Removes all key-value pairs in the map.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let stats = DashMap::new();
    /// stats.insert("Goals", 4);
    /// assert!(!stats.is_empty());
    /// stats.clear();
    /// assert!(stats.is_empty());
    /// ```
    pub fn clear(&self) {
        self._clear();
    }

    /// Returns how many key-value pairs the map can store without reallocating.
    ///
    /// **Locking behaviour:** May deadlock if called when holding a mutable reference into the map.
    pub fn capacity(&self) -> usize {
        self._capacity()
    }

    /// Modify a specific value according to a function.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let stats = DashMap::new();
    /// stats.insert("Goals", 4);
    /// stats.alter("Goals", |_, v| v * 2);
    /// assert_eq!(*stats.get("Goals").unwrap(), 8);
    /// ```
    ///
    /// # Panics
    ///
    /// If the given closure panics, then `alter` will abort the process
    pub fn alter<Q>(&self, key: &Q, f: impl FnOnce(&K, V) -> V)
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self._alter(key, f);
    }

    /// Modify every value in the map according to a function.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let stats = DashMap::new();
    /// stats.insert("Wins", 4);
    /// stats.insert("Losses", 2);
    /// stats.alter_all(|_, v| v + 1);
    /// assert_eq!(*stats.get("Wins").unwrap(), 5);
    /// assert_eq!(*stats.get("Losses").unwrap(), 3);
    /// ```
    ///
    /// # Panics
    ///
    /// If the given closure panics, then `alter_all` will abort the process
    pub fn alter_all(&self, f: impl FnMut(&K, V) -> V) {
        self._alter_all(f);
    }

    /// Checks if the map contains a specific key.
    ///
    /// **Locking behaviour:** May deadlock if called when holding a mutable reference into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashmap::DashMap;
    ///
    /// let team_sizes = DashMap::new();
    /// team_sizes.insert("Dakota Cherries", 23);
    /// assert!(team_sizes.contains_key("Dakota Cherries"));
    /// ```
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self._contains_key(key)
    }

    /// Advanced entry API that tries to mimic `std::collections::HashMap`.
    /// See the documentation on `dashmap::mapref::entry` for more details.
    ///
    /// **Locking behaviour:** May deadlock if called when holding any sort of reference into the map.
    pub fn entry(&'a self, key: K) -> Entry<'a, K, V, S> {
        self._entry(key)
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a, S: 'a + BuildHasher + Clone> Map<'a, K, V, S>
    for DashMap<K, V, S>
{
    fn _shard_count(&self) -> usize {
        self.shards.len()
    }

    unsafe fn _get_read_shard(&'a self, i: usize) -> &'a HashMap<K, V, S> {
        debug_assert!(i < self.shards.len());

        self.shards.get_unchecked(i).get()
    }

    unsafe fn _yield_read_shard(&'a self, i: usize) -> RwLockReadGuard<'a, HashMap<K, V, S>> {
        debug_assert!(i < self.shards.len());

        self.shards.get_unchecked(i).read()
    }

    unsafe fn _yield_write_shard(&'a self, i: usize) -> RwLockWriteGuard<'a, HashMap<K, V, S>> {
        debug_assert!(i < self.shards.len());

        self.shards.get_unchecked(i).write()
    }

    fn _insert(&self, key: K, value: V) -> Option<V> {
        let hash = self.hash_usize(&key);

        let idx = self.determine_shard(hash);

        let mut shard = unsafe { self._yield_write_shard(idx) };

        shard
            .insert(key, SharedValue::new(value))
            .map(|v| v.into_inner())
    }

    fn _remove<Q>(&self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hash_usize(&key);

        let idx = self.determine_shard(hash);

        let mut shard = unsafe { self._yield_write_shard(idx) };

        shard.remove_entry(key).map(|(k, v)| (k, v.into_inner()))
    }

    fn _remove_if<Q>(&self, key: &Q, f: impl FnOnce(&K, &V) -> bool) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hash_usize(&key);

        let idx = self.determine_shard(hash);

        let mut shard = unsafe { self._yield_write_shard(idx) };

        if let Some((k, v)) = shard.get_key_value(key) {
            if f(k, v.get()) {
                shard.remove_entry(key).map(|(k, v)| (k, v.into_inner()))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn _iter(&'a self) -> Iter<'a, K, V, S, DashMap<K, V, S>> {
        Iter::new(self)
    }

    fn _iter_mut(&'a self) -> IterMut<'a, K, V, S, DashMap<K, V, S>> {
        IterMut::new(self)
    }

    fn _get<Q>(&'a self, key: &Q) -> Option<Ref<'a, K, V, S>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hash_usize(&key);

        let idx = self.determine_shard(hash);

        let shard = unsafe { self._yield_read_shard(idx) };

        if let Some((kptr, vptr)) = shard.get_key_value(key) {
            unsafe {
                let kptr = util::change_lifetime_const(kptr);

                let vptr = util::change_lifetime_const(vptr);

                Some(Ref::new(shard, kptr, vptr.get()))
            }
        } else {
            None
        }
    }

    fn _get_mut<Q>(&'a self, key: &Q) -> Option<RefMut<'a, K, V, S>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hash_usize(&key);

        let idx = self.determine_shard(hash);

        let shard = unsafe { self._yield_write_shard(idx) };

        if let Some((kptr, vptr)) = shard.get_key_value(key) {
            unsafe {
                let kptr = util::change_lifetime_const(kptr);

                let vptr = &mut *vptr.as_ptr();

                Some(RefMut::new(shard, kptr, vptr))
            }
        } else {
            None
        }
    }

    fn _shrink_to_fit(&self) {
        self.shards.iter().for_each(|s| s.write().shrink_to_fit());
    }

    fn _retain(&self, mut f: impl FnMut(&K, &mut V) -> bool) {
        self.shards
            .iter()
            .for_each(|s| s.write().retain(|k, v| f(k, v.get_mut())));
    }

    fn _len(&self) -> usize {
        self.shards.iter().map(|s| s.read().len()).sum()
    }

    fn _capacity(&self) -> usize {
        self.shards.iter().map(|s| s.read().capacity()).sum()
    }

    fn _alter<Q>(&self, key: &Q, f: impl FnOnce(&K, V) -> V)
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(mut r) = self.get_mut(key) {
            util::map_in_place_2(r.pair_mut(), f);
        }
    }

    fn _alter_all(&self, mut f: impl FnMut(&K, V) -> V) {
        self.shards.iter().for_each(|s| {
            s.write()
                .iter_mut()
                .for_each(|(k, v)| util::map_in_place_2((k, v.get_mut()), &mut f));
        });
    }

    fn _entry(&'a self, key: K) -> Entry<'a, K, V, S> {
        let hash = self.hash_usize(&key);

        let idx = self.determine_shard(hash);

        let shard = unsafe { self._yield_write_shard(idx) };

        if let Some((kptr, vptr)) = shard.get_key_value(&key) {
            unsafe {
                let kptr = util::change_lifetime_const(kptr);

                let vptr = &mut *vptr.as_ptr();

                Entry::Occupied(OccupiedEntry::new(shard, key, (kptr, vptr)))
            }
        } else {
            Entry::Vacant(VacantEntry::new(shard, key))
        }
    }

    fn _hasher(&self) -> S {
        self.hasher.clone()
    }
}

impl<K: Eq + Hash + fmt::Debug, V: fmt::Debug, S: BuildHasher + Clone> fmt::Debug
    for DashMap<K, V, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pmap = f.debug_map();

        for r in self {
            let (k, v) = r.pair();

            pmap.entry(k, v);
        }

        pmap.finish()
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a, S: BuildHasher + Clone> Shl<(K, V)> for &'a DashMap<K, V, S> {
    type Output = Option<V>;

    fn shl(self, pair: (K, V)) -> Self::Output {
        self.insert(pair.0, pair.1)
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a, S: BuildHasher + Clone, Q> Shr<&Q> for &'a DashMap<K, V, S>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    type Output = Ref<'a, K, V, S>;

    fn shr(self, key: &Q) -> Self::Output {
        self.get(key).unwrap()
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a, S: BuildHasher + Clone, Q> BitOr<&Q> for &'a DashMap<K, V, S>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    type Output = RefMut<'a, K, V, S>;

    fn bitor(self, key: &Q) -> Self::Output {
        self.get_mut(key).unwrap()
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a, S: BuildHasher + Clone, Q> Sub<&Q> for &'a DashMap<K, V, S>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    type Output = Option<(K, V)>;

    fn sub(self, key: &Q) -> Self::Output {
        self.remove(key)
    }
}

impl<'a, K: 'a + Eq + Hash, V: 'a, S: BuildHasher + Clone, Q> BitAnd<&Q> for &'a DashMap<K, V, S>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    type Output = bool;

    fn bitand(self, key: &Q) -> Self::Output {
        self.contains_key(key)
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher + Clone> IntoIterator for DashMap<K, V, S> {
    type Item = (K, V);

    type IntoIter = OwningIter<K, V, S>;

    fn into_iter(self) -> Self::IntoIter {
        OwningIter::new(self)
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher + Clone> IntoIterator for &'a DashMap<K, V, S> {
    type Item = RefMulti<'a, K, V, S>;

    type IntoIter = Iter<'a, K, V, S, DashMap<K, V, S>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K: Eq + Hash, V, S: BuildHasher + Clone> Extend<(K, V)> for DashMap<K, V, S> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, intoiter: I) {
        for pair in intoiter.into_iter() {
            self.insert(pair.0, pair.1);
        }
    }
}

impl<K: Eq + Hash, V> FromIterator<(K, V)> for DashMap<K, V, RandomState> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(intoiter: I) -> Self {
        let mut map = DashMap::new();

        map.extend(intoiter);

        map
    }
}

#[cfg(test)]
mod tests {
    use crate::DashMap;
    use std::collections::hash_map::RandomState;

    #[test]
    fn test_basic() {
        let dm = DashMap::new();

        dm.insert(0, 0);

        assert_eq!(dm.get(&0).unwrap().value(), &0);
    }

    #[test]
    fn test_default() {
        let dm: DashMap<u32, u32> = DashMap::default();

        dm.insert(0, 0);

        assert_eq!(dm.get(&0).unwrap().value(), &0);
    }

    #[test]
    fn test_multiple_hashes() {
        let dm: DashMap<u32, u32> = DashMap::default();

        for i in 0..100 {
            dm.insert(0, i);

            dm.insert(i, i);
        }

        for i in 1..100 {
            let r = dm.get(&i).unwrap();

            assert_eq!(i, *r.value());

            assert_eq!(i, *r.key());
        }

        let r = dm.get(&0).unwrap();

        assert_eq!(99, *r.value());
    }

    #[test]
    fn test_more_complex_values() {
        #[derive(Hash, PartialEq, Debug, Clone)]

        struct T0 {
            s: String,
            u: u8,
        }

        let dm = DashMap::new();

        let range = 0..10;

        for i in range {
            let t = T0 {
                s: i.to_string(),
                u: i as u8,
            };

            dm.insert(i, t.clone());

            assert_eq!(&t, dm.get(&i).unwrap().value());
        }
    }

    #[test]
    fn test_different_hashers_randomstate() {
        let dm_hm_default: DashMap<u32, u32, RandomState> =
            DashMap::with_hasher(RandomState::new());

        for i in 0..10 {
            dm_hm_default.insert(i, i);

            assert_eq!(i, *dm_hm_default.get(&i).unwrap().value());
        }
    }
}
