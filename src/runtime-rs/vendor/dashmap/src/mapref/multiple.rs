use crate::lock::{RwLockReadGuard, RwLockWriteGuard};
use crate::HashMap;
use core::hash::BuildHasher;
use core::hash::Hash;
use core::ops::{Deref, DerefMut};
use std::collections::hash_map::RandomState;
use std::sync::Arc;

// -- Shared
pub struct RefMulti<'a, K, V, S = RandomState> {
    _guard: Arc<RwLockReadGuard<'a, HashMap<K, V, S>>>,
    k: &'a K,
    v: &'a V,
}

unsafe impl<'a, K: Eq + Hash + Send, V: Send, S: BuildHasher> Send for RefMulti<'a, K, V, S> {}

unsafe impl<'a, K: Eq + Hash + Send + Sync, V: Send + Sync, S: BuildHasher> Sync
    for RefMulti<'a, K, V, S>
{
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> RefMulti<'a, K, V, S> {
    pub(crate) fn new(
        guard: Arc<RwLockReadGuard<'a, HashMap<K, V, S>>>,
        k: &'a K,
        v: &'a V,
    ) -> Self {
        Self {
            _guard: guard,
            k,
            v,
        }
    }

    pub fn key(&self) -> &K {
        self.k
    }

    pub fn value(&self) -> &V {
        self.v
    }

    pub fn pair(&self) -> (&K, &V) {
        (self.k, self.v)
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> Deref for RefMulti<'a, K, V, S> {
    type Target = V;

    fn deref(&self) -> &V {
        self.value()
    }
}

// --
// -- Unique
pub struct RefMutMulti<'a, K, V, S = RandomState> {
    _guard: Arc<RwLockWriteGuard<'a, HashMap<K, V, S>>>,
    k: &'a K,
    v: &'a mut V,
}

unsafe impl<'a, K: Eq + Hash + Send, V: Send, S: BuildHasher> Send for RefMutMulti<'a, K, V, S> {}

unsafe impl<'a, K: Eq + Hash + Send + Sync, V: Send + Sync, S: BuildHasher> Sync
    for RefMutMulti<'a, K, V, S>
{
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> RefMutMulti<'a, K, V, S> {
    pub(crate) fn new(
        guard: Arc<RwLockWriteGuard<'a, HashMap<K, V, S>>>,
        k: &'a K,
        v: &'a mut V,
    ) -> Self {
        Self {
            _guard: guard,
            k,
            v,
        }
    }

    pub fn key(&self) -> &K {
        self.k
    }

    pub fn value(&self) -> &V {
        self.v
    }

    pub fn value_mut(&mut self) -> &mut V {
        self.v
    }

    pub fn pair(&self) -> (&K, &V) {
        (self.k, self.v)
    }

    pub fn pair_mut(&mut self) -> (&K, &mut V) {
        (self.k, self.v)
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> Deref for RefMutMulti<'a, K, V, S> {
    type Target = V;

    fn deref(&self) -> &V {
        self.value()
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> DerefMut for RefMutMulti<'a, K, V, S> {
    fn deref_mut(&mut self) -> &mut V {
        self.value_mut()
    }
}

// --
