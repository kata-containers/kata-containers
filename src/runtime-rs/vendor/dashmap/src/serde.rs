use crate::{DashMap, DashSet};
use core::fmt;
use core::hash::Hash;
use core::marker::PhantomData;
use serde::de::{Deserialize, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde::Deserializer;

pub struct DashMapVisitor<K, V> {
    marker: PhantomData<fn() -> DashMap<K, V>>,
}

impl<K, V> DashMapVisitor<K, V>
where
    K: Eq + Hash,
{
    fn new() -> Self {
        DashMapVisitor {
            marker: PhantomData,
        }
    }
}

impl<'de, K, V> Visitor<'de> for DashMapVisitor<K, V>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
{
    type Value = DashMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a DashMap")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let map = DashMap::with_capacity(access.size_hint().unwrap_or(0));

        while let Some((key, value)) = access.next_entry()? {
            map.insert(key, value);
        }

        Ok(map)
    }
}

impl<'de, K, V> Deserialize<'de> for DashMap<K, V>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(DashMapVisitor::<K, V>::new())
    }
}

impl<K, V> Serialize for DashMap<K, V>
where
    K: Serialize + Eq + Hash,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;

        for ref_multi in self.iter() {
            map.serialize_entry(ref_multi.key(), ref_multi.value())?;
        }

        map.end()
    }
}

pub struct DashSetVisitor<K> {
    marker: PhantomData<fn() -> DashSet<K>>,
}

impl<K> DashSetVisitor<K>
where
    K: Eq + Hash,
{
    fn new() -> Self {
        DashSetVisitor {
            marker: PhantomData,
        }
    }
}

impl<'de, K> Visitor<'de> for DashSetVisitor<K>
where
    K: Deserialize<'de> + Eq + Hash,
{
    type Value = DashSet<K>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a DashSet")
    }

    fn visit_seq<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: SeqAccess<'de>,
    {
        let map = DashSet::with_capacity(access.size_hint().unwrap_or(0));

        while let Some(key) = access.next_element()? {
            map.insert(key);
        }

        Ok(map)
    }
}

impl<'de, K> Deserialize<'de> for DashSet<K>
where
    K: Deserialize<'de> + Eq + Hash,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DashSetVisitor::<K>::new())
    }
}

impl<K> Serialize for DashSet<K>
where
    K: Serialize + Eq + Hash,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;

        for ref_multi in self.iter() {
            seq.serialize_element(ref_multi.key())?;
        }

        seq.end()
    }
}
