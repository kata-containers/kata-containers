// Copyright (c) 2016 multimap developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Serde trait implementations for MultiMap

extern crate serde;

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

use self::serde::{Deserialize, Deserializer, Serialize, Serializer};
use self::serde::de::{MapAccess, Visitor};

use MultiMap;


impl<K, V> Serialize for MultiMap<K, V>
    where K: Serialize + Eq + Hash,
          V: Serialize
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        self.inner.serialize(serializer)
    }
}

impl<K, V> MultiMapVisitor<K, V>
    where K: Hash + Eq
{
    fn new() -> Self {
        MultiMapVisitor {
            marker: PhantomData
        }
    }
}

struct MultiMapVisitor<K, V> {
    marker: PhantomData<MultiMap<K, V>>
}

impl<'a, K, V> Visitor<'a> for MultiMapVisitor<K, V>
    where K: Deserialize<'a> + Eq + Hash,
          V: Deserialize<'a>
{
    type Value = MultiMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("expected a map")
    }

    fn visit_map<M>(self, mut visitor: M) -> Result<Self::Value, M::Error>
        where M: MapAccess<'a>
    {
        let mut values = MultiMap::with_capacity(visitor.size_hint().unwrap_or(0));

        while let Some((key, value)) = visitor.next_entry()? {
            values.inner.insert(key, value);
        }

        Ok(values)
    }
}

impl<'a, K, V> Deserialize<'a> for MultiMap<K, V>
    where K: Deserialize<'a> + Eq + Hash,
          V: Deserialize<'a>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_map(MultiMapVisitor::<K, V>::new())
    }
}


#[cfg(test)]
mod tests {

    extern crate serde_test;

    use self::serde_test::{Token, assert_tokens};

    use super::*;

    #[test]
    fn test_empty() {
        let map = MultiMap::<char, u8>::new();

        assert_tokens(&map, &[
            Token::Map { len: Some(0) },
            Token::MapEnd,
        ]);
    }

    #[test]
    fn test_single() {
        let mut map = MultiMap::<char, u8>::new();
        map.insert('x', 1);

        assert_tokens(&map, &[
            Token::Map { len: Some(1) },
            Token::Char('x'),
            Token::Seq { len: Some(1) },
            Token::U8(1),
            Token::SeqEnd,
            Token::MapEnd,
        ]);
    }

    #[test]
    fn test_multiple() {
        let mut map = MultiMap::<char, u8>::new();
        map.insert('x', 1);
        map.insert('x', 3);
        map.insert('x', 1);
        map.insert('x', 5);

        assert_tokens(&map, &[
            Token::Map { len: Some(1) },
            Token::Char('x'),
            Token::Seq { len: Some(4) },
            Token::U8(1),
            Token::U8(3),
            Token::U8(1),
            Token::U8(5),
            Token::SeqEnd,
            Token::MapEnd,
        ]);
    }
}
