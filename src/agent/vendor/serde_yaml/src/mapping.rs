//! A YAML mapping and its iterator types.

use crate::Value;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::ops::{Index, IndexMut};

/// A YAML mapping in which the keys and values are both `serde_yaml::Value`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Mapping {
    map: IndexMap<Value, Value>,
}

impl Mapping {
    /// Creates an empty YAML map.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty YAML map with the given initial capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Mapping {
            map: IndexMap::with_capacity(capacity),
        }
    }

    /// Reserves capacity for at least `additional` more elements to be inserted
    /// into the map. The map may reserve more space to avoid frequent
    /// allocations.
    ///
    /// # Panics
    ///
    /// Panics if the new allocation size overflows `usize`.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }

    /// Shrinks the capacity of the map as much as possible. It will drop down
    /// as much as possible while maintaining the internal rules and possibly
    /// leaving some space in accordance with the resize policy.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }

    /// Inserts a key-value pair into the map. If the key already existed, the
    /// old value is returned.
    #[inline]
    pub fn insert(&mut self, k: Value, v: Value) -> Option<Value> {
        self.map.insert(k, v)
    }

    /// Checks if the map contains the given key.
    #[inline]
    pub fn contains_key(&self, k: &Value) -> bool {
        self.map.contains_key(k)
    }

    /// Returns the value corresponding to the key in the map.
    #[inline]
    pub fn get(&self, k: &Value) -> Option<&Value> {
        self.map.get(k)
    }

    /// Returns the mutable reference corresponding to the key in the map.
    #[inline]
    pub fn get_mut(&mut self, k: &Value) -> Option<&mut Value> {
        self.map.get_mut(k)
    }

    /// Gets the given key’s corresponding entry in the map for insertion and/or
    /// in-place manipulation.
    #[inline]
    pub fn entry(&mut self, k: Value) -> Entry {
        match self.map.entry(k) {
            indexmap::map::Entry::Occupied(occupied) => Entry::Occupied(OccupiedEntry { occupied }),
            indexmap::map::Entry::Vacant(vacant) => Entry::Vacant(VacantEntry { vacant }),
        }
    }

    /// Removes and returns the value corresponding to the key from the map.
    #[inline]
    pub fn remove(&mut self, k: &Value) -> Option<Value> {
        self.map.remove(k)
    }

    /// Returns the maximum number of key-value pairs the map can hold without
    /// reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// Returns the number of key-value pairs in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns whether the map is currently empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clears the map of all key-value pairs.
    #[inline]
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Returns a double-ended iterator visiting all key-value pairs in order of
    /// insertion. Iterator element type is `(&'a Value, &'a Value)`.
    #[inline]
    pub fn iter(&self) -> Iter {
        Iter {
            iter: self.map.iter(),
        }
    }

    /// Returns a double-ended iterator visiting all key-value pairs in order of
    /// insertion. Iterator element type is `(&'a Value, &'a mut ValuE)`.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut {
        IterMut {
            iter: self.map.iter_mut(),
        }
    }
}

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for Mapping {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the kv pairs in a way that is not sensitive to their order.
        let mut xor = 0;
        for (k, v) in self {
            let mut hasher = DefaultHasher::new();
            k.hash(&mut hasher);
            v.hash(&mut hasher);
            xor ^= hasher.finish();
        }
        xor.hash(state);
    }
}

impl PartialOrd for Mapping {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_entries = Vec::from_iter(self);
        let mut other_entries = Vec::from_iter(other);

        // Sort in an arbitrary order that is consistent with Value's PartialOrd
        // impl.
        fn total_cmp(a: &Value, b: &Value) -> Ordering {
            match (a, b) {
                (Value::Null, Value::Null) => Ordering::Equal,
                (Value::Null, _) => Ordering::Less,
                (_, Value::Null) => Ordering::Greater,

                (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
                (Value::Bool(_), _) => Ordering::Less,
                (_, Value::Bool(_)) => Ordering::Greater,

                (Value::Number(a), Value::Number(b)) => a.total_cmp(b),
                (Value::Number(_), _) => Ordering::Less,
                (_, Value::Number(_)) => Ordering::Greater,

                (Value::String(a), Value::String(b)) => a.cmp(b),
                (Value::String(_), _) => Ordering::Less,
                (_, Value::String(_)) => Ordering::Greater,

                (Value::Sequence(a), Value::Sequence(b)) => iter_cmp_by(a, b, total_cmp),
                (Value::Sequence(_), _) => Ordering::Less,
                (_, Value::Sequence(_)) => Ordering::Greater,

                (Value::Mapping(a), Value::Mapping(b)) => {
                    iter_cmp_by(a, b, |(ak, av), (bk, bv)| {
                        total_cmp(ak, bk).then_with(|| total_cmp(av, bv))
                    })
                }
            }
        }

        fn iter_cmp_by<I, F>(this: I, other: I, mut cmp: F) -> Ordering
        where
            I: IntoIterator,
            F: FnMut(I::Item, I::Item) -> Ordering,
        {
            let mut this = this.into_iter();
            let mut other = other.into_iter();

            loop {
                let x = match this.next() {
                    None => {
                        if other.next().is_none() {
                            return Ordering::Equal;
                        } else {
                            return Ordering::Less;
                        }
                    }
                    Some(val) => val,
                };

                let y = match other.next() {
                    None => return Ordering::Greater,
                    Some(val) => val,
                };

                match cmp(x, y) {
                    Ordering::Equal => {}
                    non_eq => return non_eq,
                }
            }
        }

        // While sorting by map key, we get to assume that no two keys are
        // equal, otherwise they wouldn't both be in the map. This is not a safe
        // assumption outside of this situation.
        let total_cmp = |&(a, _): &_, &(b, _): &_| total_cmp(a, b);
        self_entries.sort_by(total_cmp);
        other_entries.sort_by(total_cmp);
        self_entries.partial_cmp(&other_entries)
    }
}

impl<'a> Index<&'a Value> for Mapping {
    type Output = Value;
    #[inline]
    fn index(&self, index: &'a Value) -> &Value {
        self.map.index(index)
    }
}

impl<'a> IndexMut<&'a Value> for Mapping {
    #[inline]
    fn index_mut(&mut self, index: &'a Value) -> &mut Value {
        self.map.index_mut(index)
    }
}

impl Extend<(Value, Value)> for Mapping {
    #[inline]
    fn extend<I: IntoIterator<Item = (Value, Value)>>(&mut self, iter: I) {
        self.map.extend(iter);
    }
}

impl FromIterator<(Value, Value)> for Mapping {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (Value, Value)>>(iter: I) -> Self {
        Mapping {
            map: IndexMap::from_iter(iter),
        }
    }
}

macro_rules! delegate_iterator {
    (($name:ident $($generics:tt)*) => $item:ty) => {
        impl $($generics)* Iterator for $name $($generics)* {
            type Item = $item;
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next()
            }
            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
        }

        impl $($generics)* ExactSizeIterator for $name $($generics)* {
            #[inline]
            fn len(&self) -> usize {
                self.iter.len()
            }
        }
    }
}

/// Iterator over `&serde_yaml::Mapping`.
pub struct Iter<'a> {
    iter: indexmap::map::Iter<'a, Value, Value>,
}

delegate_iterator!((Iter<'a>) => (&'a Value, &'a Value));

impl<'a> IntoIterator for &'a Mapping {
    type Item = (&'a Value, &'a Value);
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.map.iter(),
        }
    }
}

/// Iterator over `&mut serde_yaml::Mapping`.
pub struct IterMut<'a> {
    iter: indexmap::map::IterMut<'a, Value, Value>,
}

delegate_iterator!((IterMut<'a>) => (&'a Value, &'a mut Value));

impl<'a> IntoIterator for &'a mut Mapping {
    type Item = (&'a Value, &'a mut Value);
    type IntoIter = IterMut<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            iter: self.map.iter_mut(),
        }
    }
}

/// Iterator over `serde_yaml::Mapping` by value.
pub struct IntoIter {
    iter: indexmap::map::IntoIter<Value, Value>,
}

delegate_iterator!((IntoIter) => (Value, Value));

impl IntoIterator for Mapping {
    type Item = (Value, Value);
    type IntoIter = IntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            iter: self.map.into_iter(),
        }
    }
}

/// Entry for an existing key-value pair or a vacant location to insert one.
pub enum Entry<'a> {
    /// Existing slot with equivalent key.
    Occupied(OccupiedEntry<'a>),
    /// Vacant slot (no equivalent key in the map).
    Vacant(VacantEntry<'a>),
}

/// A view into an occupied entry in a [`Mapping`]. It is part of the [`Entry`]
/// enum.
pub struct OccupiedEntry<'a> {
    occupied: indexmap::map::OccupiedEntry<'a, Value, Value>,
}

/// A view into a vacant entry in a [`Mapping`]. It is part of the [`Entry`]
/// enum.
pub struct VacantEntry<'a> {
    vacant: indexmap::map::VacantEntry<'a, Value, Value>,
}

impl<'a> Entry<'a> {
    /// Returns a reference to this entry's key.
    pub fn key(&self) -> &Value {
        match self {
            Entry::Vacant(e) => e.key(),
            Entry::Occupied(e) => e.key(),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a mutable reference to the value in the entry.
    pub fn or_insert(self, default: Value) -> &'a mut Value {
        match self {
            Entry::Vacant(entry) => entry.insert(default),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty, and returns a mutable reference to the value in the
    /// entry.
    pub fn or_insert_with<F>(self, default: F) -> &'a mut Value
    where
        F: FnOnce() -> Value,
    {
        match self {
            Entry::Vacant(entry) => entry.insert(default()),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut Value),
    {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
}

impl<'a> OccupiedEntry<'a> {
    /// Gets a reference to the key in the entry.
    #[inline]
    pub fn key(&self) -> &Value {
        self.occupied.key()
    }

    /// Gets a reference to the value in the entry.
    #[inline]
    pub fn get(&self) -> &Value {
        self.occupied.get()
    }

    /// Gets a mutable reference to the value in the entry.
    #[inline]
    pub fn get_mut(&mut self) -> &mut Value {
        self.occupied.get_mut()
    }

    /// Converts the entry into a mutable reference to its value.
    #[inline]
    pub fn into_mut(self) -> &'a mut Value {
        self.occupied.into_mut()
    }

    /// Sets the value of the entry with the `OccupiedEntry`'s key, and returns
    /// the entry's old value.
    #[inline]
    pub fn insert(&mut self, value: Value) -> Value {
        self.occupied.insert(value)
    }

    /// Takes the value of the entry out of the map, and returns it.
    #[inline]
    pub fn remove(self) -> Value {
        self.occupied.swap_remove()
    }
}

impl<'a> VacantEntry<'a> {
    /// Gets a reference to the key that would be used when inserting a value
    /// through the VacantEntry.
    #[inline]
    pub fn key(&self) -> &Value {
        self.vacant.key()
    }

    /// Sets the value of the entry with the VacantEntry's key, and returns a
    /// mutable reference to it.
    #[inline]
    pub fn insert(self, value: Value) -> &'a mut Value {
        self.vacant.insert(value)
    }
}

impl Serialize for Mapping {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map_serializer = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self {
            map_serializer.serialize_entry(k, v)?;
        }
        map_serializer.end()
    }
}

impl<'de> Deserialize<'de> for Mapping {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Mapping;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a YAML mapping")
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Mapping::new())
            }

            #[inline]
            fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut values = Mapping::new();
                while let Some((k, v)) = visitor.next_entry()? {
                    values.insert(k, v);
                }
                Ok(values)
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}
