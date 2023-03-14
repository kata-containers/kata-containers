// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

//! Struct MultikeyBTreeMap implementation used by passthrough.

use std::borrow::Borrow;
use std::collections::BTreeMap;

/// A BTreeMap that supports 2 types of keys per value. All the usual restrictions and warnings for
/// `std::collections::BTreeMap` also apply to this struct. Additionally, there is a 1:n
/// relationship between the 2 key types: For each `K1` in the map, any number of `K2` may exist;
/// but each `K2` only has exactly one `K1` associated with it.
#[derive(Default)]
pub struct MultikeyBTreeMap<K1, K2, V>
where
    K1: Ord,
    K2: Ord,
{
    // We need to keep a copy of the second keys in the main map so that we can remove entries using
    // just the main key. Otherwise we would require the caller to provide all keys when calling
    // `remove`.
    main: BTreeMap<K1, (Vec<K2>, V)>,
    alt: BTreeMap<K2, K1>,
}

impl<K1, K2, V> MultikeyBTreeMap<K1, K2, V>
where
    K1: Clone + Ord,
    K2: Clone + Ord,
{
    /// Create a new empty MultikeyBTreeMap.
    pub fn new() -> Self {
        MultikeyBTreeMap {
            main: BTreeMap::default(),
            alt: BTreeMap::default(),
        }
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of `K1``, but the ordering on the borrowed form must match
    /// the ordering on `K1`.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K1: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.main.get(key).map(|(_, v)| v)
    }

    /// Returns a reference to the value corresponding to an alternate key.
    ///
    /// The key may be any borrowed form of the `K2``, but the ordering on the borrowed form must
    /// match the ordering on `K2`.
    ///
    /// Note that this method performs 2 lookups: one to get the main key and another to get the
    /// value associated with that key. For best performance callers should prefer the `get` method
    /// over this method whenever possible as `get` only needs to perform one lookup.
    pub fn get_alt<Q2>(&self, key: &Q2) -> Option<&V>
    where
        K2: Borrow<Q2>,
        Q2: Ord + ?Sized,
    {
        if let Some(k) = self.alt.get(key) {
            self.get(k)
        } else {
            None
        }
    }

    /// Inserts a new entry into the map with the given main key and value.
    ///
    /// If there already was an entry present with the given key, then the value is updated,
    /// all alternate keys pointing to the main key are removed, and the old value is returned.
    /// Otherwise, returns `None`.
    pub fn insert(&mut self, k1: K1, v: V) -> Option<V> {
        self.main.insert(k1, (vec![], v)).map(|(k2s, old_v)| {
            for k2 in &k2s {
                self.alt.remove(k2);
            }
            old_v
        })
    }

    /// Adds an alternate key for an existing main key.
    ///
    /// If the given alternate key was present already, then the main key it points to is updated,
    /// and that previous main key is returned.
    /// Otherwise, returns `None`.
    pub fn insert_alt(&mut self, k2: K2, k1: K1) -> Option<K1> {
        // `k1` must exist, so we can .unwrap()
        self.main.get_mut(&k1).unwrap().0.push(k2.clone());

        if let Some(old_k1) = self.alt.insert(k2.clone(), k1) {
            if let Some((old_k1_v2s, _)) = self.main.get_mut(&old_k1) {
                if let Some(i) = old_k1_v2s.iter().position(|x| *x == k2) {
                    old_k1_v2s.remove(i);
                }
            }

            Some(old_k1)
        } else {
            None
        }
    }

    /// Remove a key from the map, returning the value associated with that key if it was previously
    /// in the map.
    ///
    /// The key may be any borrowed form of `K1``, but the ordering on the borrowed form must match
    /// the ordering on `K1`.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K1: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.main.remove(key).map(|(k2s, v)| {
            for k2 in &k2s {
                self.alt.remove(k2);
            }
            v
        })
    }

    #[allow(dead_code)]
    pub fn remove_alt(&mut self, key: &K2) -> Option<K1> {
        if let Some(k1) = self.alt.remove(key) {
            if let Some((k1_v2s, _)) = self.main.get_mut(&k1) {
                if let Some(i) = k1_v2s.iter().position(|x| *x == *key) {
                    k1_v2s.remove(i);
                }
            }

            Some(k1)
        } else {
            None
        }
    }

    /// Clears the map, removing all values.
    pub fn clear(&mut self) {
        self.alt.clear();
        self.main.clear()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2 = 0x1a04_ce4b_8329_14fe;
        let val = 0xf4e3_c360;
        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2, k1).is_none());

        assert_eq!(*m.get(&k1).expect("failed to look up main key"), val);
        assert_eq!(*m.get_alt(&k2).expect("failed to look up alt key"), val);
    }

    #[test]
    fn get_multi_alt() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2a = 0x1a04_ce4b_8329_14fe;
        let k2b = 0x6825_a60b_61ac_b333;
        let val = 0xf4e3_c360;

        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2a, k1).is_none());
        assert!(m.insert_alt(k2b, k1).is_none());

        assert_eq!(*m.get_alt(&k2a).expect("failed to look up alt key A"), val);
        assert_eq!(*m.get_alt(&k2b).expect("failed to look up alt key B"), val);
    }

    #[test]
    fn update_main_key() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2 = 0x1a04_ce4b_8329_14fe;
        let val = 0xf4e3_c360;
        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2, k1).is_none());

        let new_k1 = 0x3add_f8f8_c7c5_df5e;
        let val2 = 0x7389_f8a7;
        assert!(m.insert(new_k1, val2).is_none());
        assert_eq!(
            m.insert_alt(k2, new_k1)
                .expect("failed to update main key to which alt key points"),
            k1
        );
        assert_eq!(m.remove(&k1).expect("failed to remove old main key"), val);

        assert!(m.get(&k1).is_none());
        assert_eq!(*m.get(&new_k1).expect("failed to look up main key"), val2);
        assert_eq!(*m.get_alt(&k2).expect("failed to look up alt key"), val2);
    }

    #[test]
    fn update_alt_key() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2 = 0x1a04_ce4b_8329_14fe;
        let val = 0xf4e3_c360;
        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2, k1).is_none());

        let new_k2 = 0x6825_a60b_61ac_b333;
        let val2 = 0xbb14_8f2c;
        assert_eq!(m.insert(k1, val2).expect("failed to update value"), val);
        // Updating a main key invalidates all its alt keys
        assert!(m.insert_alt(new_k2, k1).is_none());

        assert!(m.get_alt(&k2).is_none());
        assert_eq!(*m.get(&k1).expect("failed to look up main key"), val2);
        assert_eq!(
            *m.get_alt(&new_k2).expect("failed to look up alt key"),
            val2
        );
    }

    #[test]
    fn update_value() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2 = 0x1a04_ce4b_8329_14fe;
        let val = 0xf4e3_c360;
        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2, k1).is_none());

        let val2 = 0xe42d_79ba;
        assert_eq!(m.insert(k1, val2).expect("failed to update alt key"), val);
        // Updating a main key invalidates all its alt keys
        assert!(m.insert_alt(k2, k1).is_none());

        assert_eq!(*m.get(&k1).expect("failed to look up main key"), val2);
        assert_eq!(*m.get_alt(&k2).expect("failed to look up alt key"), val2);
    }

    #[test]
    fn update_both_keys_main() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2 = 0x1a04_ce4b_8329_14fe;
        let val = 0xf4e3_c360;
        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2, k1).is_none());

        let new_k1 = 0xc980_587a_24b3_ae30;
        let new_k2 = 0x2773_c5ee_8239_45a2;
        let val2 = 0x31f4_33f9;
        assert!(m.insert(new_k1, val2).is_none());
        assert!(m.insert_alt(new_k2, new_k1).is_none());

        let val3 = 0x8da1_9cf7;
        assert_eq!(m.insert(k1, val3).expect("failed to update main key"), val);
        assert_eq!(
            m.insert_alt(new_k2, k1).expect("failed to update alt key"),
            new_k1
        );

        // We did not touch new_k1, so it should still be there
        assert_eq!(*m.get(&new_k1).expect("failed to look up main key"), val2);

        // However, we did update k1, which removed its associated alt keys
        assert!(m.get_alt(&k2).is_none());

        assert_eq!(*m.get(&k1).expect("failed to look up main key"), val3);
        assert_eq!(
            *m.get_alt(&new_k2).expect("failed to look up alt key"),
            val3
        );
    }

    #[test]
    fn update_both_keys_alt() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2 = 0x1a04_ce4b_8329_14fe;
        let val = 0xf4e3_c360;
        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2, k1).is_none());

        let new_k1 = 0xc980_587a_24b3_ae30;
        let new_k2 = 0x2773_c5ee_8239_45a2;
        let val2 = 0x31f4_33f9;
        assert!(m.insert(new_k1, val2).is_none());
        assert!(m.insert_alt(new_k2, new_k1).is_none());

        let val3 = 0x8da1_9cf7;
        assert_eq!(
            m.insert(new_k1, val3).expect("failed to update main key"),
            val2
        );
        assert_eq!(
            m.insert_alt(k2, new_k1).expect("failed to update alt key"),
            k1
        );

        // We did not touch k1, so it should still be there
        assert_eq!(*m.get(&k1).expect("failed to look up first main key"), val);

        // However, we did update new_k1, which removed its associated alt keys
        assert!(m.get_alt(&new_k2).is_none());

        assert_eq!(*m.get(&new_k1).expect("failed to look up main key"), val3);
        assert_eq!(*m.get_alt(&k2).expect("failed to look up alt key"), val3);
    }

    #[test]
    fn remove() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1 = 0xc6c8_f5e0_b13e_ed40;
        let k2 = 0x1a04_ce4b_8329_14fe;
        let val = 0xf4e3_c360;
        assert!(m.insert(k1, val).is_none());
        assert!(m.insert_alt(k2, k1).is_none());

        assert_eq!(m.remove(&k1).expect("failed to remove entry"), val);
        assert!(m.get(&k1).is_none());
        assert!(m.get_alt(&k2).is_none());
    }

    #[test]
    fn remove_multi() {
        let mut m = MultikeyBTreeMap::<u64, i64, u32>::new();

        let k1a = 0xc6c8_f5e0_b13e_ed40;
        let k1b = 0x3add_f8f8_c7c5_df5e;
        let k2a = 0x1a04_ce4b_8329_14fe;
        let k2b = 0x6825_a60b_61ac_b333;
        let val_a = 0xf4e3_c360;
        let val_b = 0xe42d_79ba;

        assert!(m.insert(k1a, val_a).is_none());
        assert!(m.insert_alt(k2a, k1a).is_none());
        assert!(m.insert_alt(k2b, k1a).is_none());

        assert!(m.insert(k1b, val_b).is_none());

        assert_eq!(
            m.insert_alt(k2b, k1b)
                .expect("failed to make second alt key point to second main key"),
            k1a
        );

        assert_eq!(
            m.remove(&k1a).expect("failed to remove first main key"),
            val_a
        );

        assert!(m.get(&k1a).is_none());
        assert!(m.get_alt(&k2a).is_none());

        assert_eq!(*m.get(&k1b).expect("failed to get second main key"), val_b);
        assert_eq!(
            *m.get_alt(&k2b).expect("failed to get second alt key"),
            val_b
        );
    }
}
