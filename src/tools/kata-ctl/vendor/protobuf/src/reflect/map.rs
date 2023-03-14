use std::collections::hash_map;
use std::collections::HashMap;
use std::hash::Hash;

use super::value::ProtobufValue;

/// Implemented for `HashMap` with appropriate keys and values
pub trait ReflectMap: 'static {
    fn reflect_iter(&self) -> ReflectMapIter;

    fn len(&self) -> usize;
}

impl<K: ProtobufValue + Eq + Hash + 'static, V: ProtobufValue + 'static> ReflectMap
    for HashMap<K, V>
{
    fn reflect_iter<'a>(&'a self) -> ReflectMapIter<'a> {
        ReflectMapIter {
            imp: Box::new(ReflectMapIterImpl::<'a, K, V> { iter: self.iter() }),
        }
    }

    fn len(&self) -> usize {
        HashMap::len(self)
    }
}

trait ReflectMapIterTrait<'a> {
    fn next(&mut self) -> Option<(&'a dyn ProtobufValue, &'a dyn ProtobufValue)>;
}

struct ReflectMapIterImpl<'a, K: Eq + Hash + 'static, V: 'static> {
    iter: hash_map::Iter<'a, K, V>,
}

impl<'a, K: ProtobufValue + Eq + Hash + 'static, V: ProtobufValue + 'static> ReflectMapIterTrait<'a>
    for ReflectMapIterImpl<'a, K, V>
{
    fn next(&mut self) -> Option<(&'a dyn ProtobufValue, &'a dyn ProtobufValue)> {
        match self.iter.next() {
            Some((k, v)) => Some((k as &dyn ProtobufValue, v as &dyn ProtobufValue)),
            None => None,
        }
    }
}

pub struct ReflectMapIter<'a> {
    imp: Box<dyn ReflectMapIterTrait<'a> + 'a>,
}

impl<'a> Iterator for ReflectMapIter<'a> {
    type Item = (&'a dyn ProtobufValue, &'a dyn ProtobufValue);

    fn next(&mut self) -> Option<(&'a dyn ProtobufValue, &'a dyn ProtobufValue)> {
        self.imp.next()
    }
}

impl<'a> IntoIterator for &'a dyn ReflectMap {
    type IntoIter = ReflectMapIter<'a>;
    type Item = (&'a dyn ProtobufValue, &'a dyn ProtobufValue);

    fn into_iter(self) -> Self::IntoIter {
        self.reflect_iter()
    }
}
