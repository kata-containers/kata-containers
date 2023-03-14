use std::slice;

use super::value::ProtobufValue;
use super::value::ReflectValueRef;
use crate::repeated::RepeatedField;

pub trait ReflectRepeated: 'static {
    fn reflect_iter(&self) -> ReflectRepeatedIter;
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> &dyn ProtobufValue;
}

impl<V: ProtobufValue + 'static> ReflectRepeated for Vec<V> {
    fn reflect_iter<'a>(&'a self) -> ReflectRepeatedIter<'a> {
        ReflectRepeatedIter {
            imp: Box::new(ReflectRepeatedIterImplSlice::<'a, V> { iter: self.iter() }),
        }
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn get(&self, index: usize) -> &dyn ProtobufValue {
        &self[index]
    }
}

// useless
impl<V: ProtobufValue + 'static> ReflectRepeated for [V] {
    fn reflect_iter<'a>(&'a self) -> ReflectRepeatedIter<'a> {
        ReflectRepeatedIter {
            imp: Box::new(ReflectRepeatedIterImplSlice::<'a, V> { iter: self.iter() }),
        }
    }

    fn len(&self) -> usize {
        <[_]>::len(self)
    }

    fn get(&self, index: usize) -> &dyn ProtobufValue {
        &self[index]
    }
}

impl<V: ProtobufValue + 'static> ReflectRepeated for RepeatedField<V> {
    fn reflect_iter<'a>(&'a self) -> ReflectRepeatedIter<'a> {
        ReflectRepeatedIter {
            imp: Box::new(ReflectRepeatedIterImplSlice::<'a, V> { iter: self.iter() }),
        }
    }

    fn len(&self) -> usize {
        RepeatedField::len(self)
    }

    fn get(&self, index: usize) -> &dyn ProtobufValue {
        &self[index]
    }
}

trait ReflectRepeatedIterTrait<'a> {
    fn next(&mut self) -> Option<&'a dyn ProtobufValue>;
}

struct ReflectRepeatedIterImplSlice<'a, V: ProtobufValue + 'static> {
    iter: slice::Iter<'a, V>,
}

impl<'a, V: ProtobufValue + 'static> ReflectRepeatedIterTrait<'a>
    for ReflectRepeatedIterImplSlice<'a, V>
{
    fn next(&mut self) -> Option<&'a dyn ProtobufValue> {
        self.iter.next().map(|v| v as &dyn ProtobufValue)
    }
}

pub struct ReflectRepeatedIter<'a> {
    imp: Box<dyn ReflectRepeatedIterTrait<'a> + 'a>,
}

impl<'a> Iterator for ReflectRepeatedIter<'a> {
    type Item = &'a dyn ProtobufValue;

    fn next(&mut self) -> Option<Self::Item> {
        self.imp.next()
    }
}

impl<'a> IntoIterator for &'a dyn ReflectRepeated {
    type IntoIter = ReflectRepeatedIter<'a>;
    type Item = &'a dyn ProtobufValue;

    fn into_iter(self) -> Self::IntoIter {
        self.reflect_iter()
    }
}

pub trait ReflectRepeatedEnum<'a> {
    fn len(&self) -> usize;

    fn get(&self, index: usize) -> ReflectValueRef<'a>;
}

pub trait ReflectRepeatedMessage<'a> {
    fn len(&self) -> usize;

    fn get(&self, index: usize) -> ReflectValueRef<'a>;
}

pub enum ReflectRepeatedRef<'a> {
    Generic(&'a dyn ReflectRepeated),
    U32(&'a [u32]),
    U64(&'a [u64]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    F32(&'a [f32]),
    F64(&'a [f64]),
    Bool(&'a [bool]),
    String(&'a [String]),
    Bytes(&'a [Vec<u8>]),
    Enum(Box<dyn ReflectRepeatedEnum<'a> + 'a>),
    Message(Box<dyn ReflectRepeatedMessage<'a> + 'a>),
}

impl<'a> ReflectRepeatedRef<'a> {
    fn len(&self) -> usize {
        match *self {
            ReflectRepeatedRef::Generic(ref r) => r.len(),
            ReflectRepeatedRef::U32(ref r) => r.len(),
            ReflectRepeatedRef::U64(ref r) => r.len(),
            ReflectRepeatedRef::I32(ref r) => r.len(),
            ReflectRepeatedRef::I64(ref r) => r.len(),
            ReflectRepeatedRef::F32(ref r) => r.len(),
            ReflectRepeatedRef::F64(ref r) => r.len(),
            ReflectRepeatedRef::Bool(ref r) => r.len(),
            ReflectRepeatedRef::String(ref r) => r.len(),
            ReflectRepeatedRef::Bytes(ref r) => r.len(),
            ReflectRepeatedRef::Enum(ref r) => r.len(),
            ReflectRepeatedRef::Message(ref r) => r.len(),
        }
    }

    fn get(&self, index: usize) -> ReflectValueRef<'a> {
        match *self {
            ReflectRepeatedRef::Generic(ref r) => r.get(index).as_ref(),
            ReflectRepeatedRef::U32(ref r) => ReflectValueRef::U32(r[index]),
            ReflectRepeatedRef::U64(ref r) => ReflectValueRef::U64(r[index]),
            ReflectRepeatedRef::I32(ref r) => ReflectValueRef::I32(r[index]),
            ReflectRepeatedRef::I64(ref r) => ReflectValueRef::I64(r[index]),
            ReflectRepeatedRef::F32(ref r) => ReflectValueRef::F32(r[index]),
            ReflectRepeatedRef::F64(ref r) => ReflectValueRef::F64(r[index]),
            ReflectRepeatedRef::Bool(ref r) => ReflectValueRef::Bool(r[index]),
            ReflectRepeatedRef::String(ref r) => ReflectValueRef::String(&r[index]),
            ReflectRepeatedRef::Bytes(ref r) => ReflectValueRef::Bytes(&r[index]),
            ReflectRepeatedRef::Enum(ref r) => r.get(index),
            ReflectRepeatedRef::Message(ref r) => r.get(index),
        }
    }
}

pub struct ReflectRepeatedRefIter<'a> {
    repeated: &'a ReflectRepeatedRef<'a>,
    pos: usize,
}

impl<'a> Iterator for ReflectRepeatedRefIter<'a> {
    type Item = ReflectValueRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.repeated.len() {
            let pos = self.pos;
            self.pos += 1;
            Some(self.repeated.get(pos))
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a ReflectRepeatedRef<'a> {
    type IntoIter = ReflectRepeatedRefIter<'a>;
    type Item = ReflectValueRef<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ReflectRepeatedRefIter {
            repeated: self,
            pos: 0,
        }
    }
}
