use std::mem;

use super::value::ProtobufValue;
use crate::singular::*;

pub trait ReflectOptional: 'static {
    fn to_option(&self) -> Option<&dyn ProtobufValue>;

    fn set_value(&mut self, value: &dyn ProtobufValue);
}

impl<V: ProtobufValue + Clone + 'static> ReflectOptional for Option<V> {
    fn to_option(&self) -> Option<&dyn ProtobufValue> {
        self.as_ref().map(|v| v as &dyn ProtobufValue)
    }

    fn set_value(&mut self, value: &dyn ProtobufValue) {
        match value.as_any().downcast_ref::<V>() {
            Some(v) => mem::replace(self, Some(v.clone())),
            None => panic!(),
        };
    }
}

impl<V: ProtobufValue + Clone + 'static> ReflectOptional for SingularField<V> {
    fn to_option(&self) -> Option<&dyn ProtobufValue> {
        self.as_ref().map(|v| v as &dyn ProtobufValue)
    }

    fn set_value(&mut self, value: &dyn ProtobufValue) {
        match value.as_any().downcast_ref::<V>() {
            Some(v) => mem::replace(self, SingularField::some(v.clone())),
            None => panic!(),
        };
    }
}

impl<V: ProtobufValue + Clone + 'static> ReflectOptional for SingularPtrField<V> {
    fn to_option(&self) -> Option<&dyn ProtobufValue> {
        self.as_ref().map(|v| v as &dyn ProtobufValue)
    }

    fn set_value(&mut self, value: &dyn ProtobufValue) {
        match value.as_any().downcast_ref::<V>() {
            Some(v) => mem::replace(self, SingularPtrField::some(v.clone())),
            None => panic!(),
        };
    }
}
