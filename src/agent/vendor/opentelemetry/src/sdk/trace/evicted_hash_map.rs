//! # Evicted Map

use crate::{Key, KeyValue, Value};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, LinkedList};

/// A hash map with a capped number of attributes that retains the most
/// recently set entries.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct EvictedHashMap {
    map: HashMap<Key, Value>,
    evict_list: LinkedList<Key>,
    max_len: u32,
    dropped_count: u32,
}

impl EvictedHashMap {
    /// Create a new `EvictedHashMap` with a given max length and capacity.
    pub fn new(max_len: u32, capacity: usize) -> Self {
        EvictedHashMap {
            map: HashMap::with_capacity(capacity),
            evict_list: LinkedList::new(),
            max_len,
            dropped_count: 0,
        }
    }

    /// Inserts a key-value pair into the map.
    pub fn insert(&mut self, item: KeyValue) {
        let KeyValue { key, value } = item;
        let mut already_exists = false;
        // Check for existing item
        match self.map.entry(key.clone()) {
            Entry::Occupied(mut occupied) => {
                occupied.insert(value);
                already_exists = true;
            }
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
        }

        if already_exists {
            self.move_key_to_front(key);
        } else {
            // Add new item
            self.evict_list.push_front(key);
        }

        // Verify size not exceeded
        if self.evict_list.len() as u32 > self.max_len {
            self.remove_oldest();
            self.dropped_count += 1;
        }
    }

    /// Returns the number of elements in the map.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the dropped attribute count
    pub fn dropped_count(&self) -> u32 {
        self.dropped_count
    }

    /// Returns a front-to-back iterator.
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.map.iter())
    }

    /// Returns a reference to the value corresponding to the key if it exists
    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.map.get(key)
    }

    fn move_key_to_front(&mut self, key: Key) {
        if self.evict_list.is_empty() {
            // If empty, push front
            self.evict_list.push_front(key);
        } else if self.evict_list.front() == Some(&key) {
            // Already the front, ignore
        } else {
            // Else split linked lists around key and combine
            let key_idx = self
                .evict_list
                .iter()
                .position(|k| k == &key)
                .expect("key must exist in evicted hash map, this is a bug");
            let mut tail = self.evict_list.split_off(key_idx);
            let item = tail.pop_front().unwrap();
            self.evict_list.push_front(item);
            self.evict_list.append(&mut tail);
        }
    }

    fn remove_oldest(&mut self) {
        if let Some(oldest_item) = self.evict_list.pop_back() {
            self.map.remove(&oldest_item);
        }
    }
}

/// An owned iterator over the entries of a `EvictedHashMap`.
#[derive(Debug)]
pub struct IntoIter(std::collections::hash_map::IntoIter<Key, Value>);

impl Iterator for IntoIter {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl IntoIterator for EvictedHashMap {
    type Item = (Key, Value);
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.map.into_iter())
    }
}

impl<'a> IntoIterator for &'a EvictedHashMap {
    type Item = (&'a Key, &'a Value);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.map.iter())
    }
}

/// An iterator over the entries of an `EvictedHashMap`.
#[derive(Debug)]
pub struct Iter<'a>(std::collections::hash_map::Iter<'a, Key, Value>);

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn insert_over_capacity_test() {
        let max_len = 10;
        let mut map = EvictedHashMap::new(max_len, max_len as usize);

        for i in 0..=max_len {
            map.insert(Key::new(i.to_string()).bool(true))
        }

        assert_eq!(map.dropped_count, 1);
        assert_eq!(map.len(), max_len as usize);
        assert_eq!(
            map.map.keys().cloned().collect::<HashSet<_>>(),
            (1..=max_len)
                .map(|i| Key::new(i.to_string()))
                .collect::<HashSet<_>>()
        );
    }
}
