#![doc(hidden)]

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use core::message_down_cast;
use core::Message;
use enums::ProtobufEnum;
use reflect::EnumValueDescriptor;
use types::*;

use repeated::RepeatedField;
use singular::SingularField;
use singular::SingularPtrField;

use super::map::ReflectMap;
use super::optional::ReflectOptional;
use super::repeated::ReflectRepeated;
use super::repeated::ReflectRepeatedEnum;
use super::repeated::ReflectRepeatedMessage;
use super::value::ProtobufValue;
use super::value::ReflectValueRef;
use super::ReflectFieldRef;

/// this trait should not be used directly, use `FieldDescriptor` instead
pub trait FieldAccessor {
    fn name_generic(&self) -> &'static str;
    fn has_field_generic(&self, m: &Message) -> bool;
    fn len_field_generic(&self, m: &Message) -> usize;
    // TODO: should it return default value or panic on unset field?
    fn get_message_generic<'a>(&self, m: &'a Message) -> &'a Message;
    fn get_enum_generic(&self, m: &Message) -> &'static EnumValueDescriptor;
    fn get_str_generic<'a>(&self, m: &'a Message) -> &'a str;
    fn get_bytes_generic<'a>(&self, m: &'a Message) -> &'a [u8];
    fn get_u32_generic(&self, m: &Message) -> u32;
    fn get_u64_generic(&self, m: &Message) -> u64;
    fn get_i32_generic(&self, m: &Message) -> i32;
    fn get_i64_generic(&self, m: &Message) -> i64;
    fn get_bool_generic(&self, m: &Message) -> bool;
    fn get_f32_generic(&self, m: &Message) -> f32;
    fn get_f64_generic(&self, m: &Message) -> f64;

    fn get_reflect<'a>(&self, m: &'a Message) -> ReflectFieldRef<'a>;
}

trait GetSingularMessage<M> {
    fn get_message<'a>(&self, m: &'a M) -> &'a Message;
}

struct GetSingularMessageImpl<M, N> {
    get: for<'a> fn(&'a M) -> &'a N,
}

impl<M: Message, N: Message + 'static> GetSingularMessage<M> for GetSingularMessageImpl<M, N> {
    fn get_message<'a>(&self, m: &'a M) -> &'a Message {
        (self.get)(m)
    }
}

trait GetSingularEnum<M> {
    fn get_enum(&self, m: &M) -> &'static EnumValueDescriptor;
}

struct GetSingularEnumImpl<M, E> {
    get: fn(&M) -> E,
}

impl<M: Message, E: ProtobufEnum> GetSingularEnum<M> for GetSingularEnumImpl<M, E> {
    fn get_enum(&self, m: &M) -> &'static EnumValueDescriptor {
        (self.get)(m).descriptor()
    }
}

trait GetRepeatedMessage<M> {
    fn len_field(&self, m: &M) -> usize;
    fn get_message_item<'a>(&self, m: &'a M, index: usize) -> &'a Message;
    fn reflect_repeated_message<'a>(&self, m: &'a M) -> Box<ReflectRepeatedMessage<'a> + 'a>;
}

trait GetRepeatedEnum<M: Message + 'static> {
    fn len_field(&self, m: &M) -> usize;
    fn get_enum_item(&self, m: &M, index: usize) -> &'static EnumValueDescriptor;
    fn reflect_repeated_enum<'a>(&self, m: &'a M) -> Box<ReflectRepeatedEnum<'a> + 'a>;
}

trait GetSetCopyFns<M> {
    fn get_field<'a>(&self, m: &'a M) -> ReflectValueRef<'a>;
}

struct GetSetCopyFnsImpl<M, V: ProtobufValue + Copy> {
    get: fn(&M) -> V,
    _set: fn(&mut M, V),
}

impl<M, V: ProtobufValue + Copy> GetSetCopyFns<M> for GetSetCopyFnsImpl<M, V> {
    fn get_field<'a>(&self, m: &'a M) -> ReflectValueRef<'a> {
        (&(self.get)(m) as &ProtobufValue).as_ref_copy()
    }
}

enum SingularGetSet<M> {
    Copy(Box<GetSetCopyFns<M>>),
    String(for<'a> fn(&'a M) -> &'a str, fn(&mut M, String)),
    Bytes(for<'a> fn(&'a M) -> &'a [u8], fn(&mut M, Vec<u8>)),
    Enum(Box<GetSingularEnum<M> + 'static>),
    Message(Box<GetSingularMessage<M> + 'static>),
}

impl<M: Message + 'static> SingularGetSet<M> {
    fn get_ref<'a>(&self, m: &'a M) -> ReflectValueRef<'a> {
        match self {
            &SingularGetSet::Copy(ref copy) => copy.get_field(m),
            &SingularGetSet::String(get, _) => ReflectValueRef::String(get(m)),
            &SingularGetSet::Bytes(get, _) => ReflectValueRef::Bytes(get(m)),
            &SingularGetSet::Enum(ref get) => ReflectValueRef::Enum(get.get_enum(m)),
            &SingularGetSet::Message(ref get) => ReflectValueRef::Message(get.get_message(m)),
        }
    }
}

trait FieldAccessor2<M, R: ?Sized>
where
    M: Message + 'static,
{
    fn get_field<'a>(&self, &'a M) -> &'a R;
    fn mut_field<'a>(&self, &'a mut M) -> &'a mut R;
}

struct MessageGetMut<M, L>
where
    M: Message + 'static,
{
    get_field: for<'a> fn(&'a M) -> &'a L,
    mut_field: for<'a> fn(&'a mut M) -> &'a mut L,
}

enum FieldAccessorFunctions<M> {
    // up to 1.0.24 optional or required
    SingularHasGetSet {
        has: fn(&M) -> bool,
        get_set: SingularGetSet<M>,
    },
    // protobuf 3 simple field
    Simple(Box<FieldAccessor2<M, ProtobufValue>>),
    // optional, required or message
    Optional(Box<FieldAccessor2<M, ReflectOptional>>),
    // repeated
    Repeated(Box<FieldAccessor2<M, ReflectRepeated>>),
    // protobuf 3 map
    Map(Box<FieldAccessor2<M, ReflectMap>>),
}

impl<M> fmt::Debug for FieldAccessorFunctions<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &FieldAccessorFunctions::SingularHasGetSet { .. } => {
                write!(f, "SingularHasGetSet {{ .. }}")
            }
            &FieldAccessorFunctions::Simple(..) => write!(f, "Simple(..)"),
            &FieldAccessorFunctions::Optional(..) => write!(f, "Optional(..)"),
            &FieldAccessorFunctions::Repeated(..) => write!(f, "Repeated(..)"),
            &FieldAccessorFunctions::Map(..) => write!(f, "Map(..)"),
        }
    }
}

struct FieldAccessorImpl<M> {
    name: &'static str,
    fns: FieldAccessorFunctions<M>,
}

impl<M: Message> FieldAccessorImpl<M> {
    fn get_value_option<'a>(&self, m: &'a M) -> Option<ReflectValueRef<'a>> {
        match self.fns {
            FieldAccessorFunctions::Repeated(..) | FieldAccessorFunctions::Map(..) => {
                panic!("repeated")
            }
            FieldAccessorFunctions::Simple(ref a) => Some(a.get_field(m).as_ref()),
            FieldAccessorFunctions::Optional(ref a) => {
                a.get_field(m).to_option().map(|v| v.as_ref())
            }
            FieldAccessorFunctions::SingularHasGetSet {
                ref has,
                ref get_set,
            } => {
                if !has(m) {
                    None
                } else {
                    Some(get_set.get_ref(m))
                }
            }
        }
    }
}

impl<M: Message + 'static> FieldAccessor for FieldAccessorImpl<M> {
    fn name_generic(&self) -> &'static str {
        self.name
    }

    fn has_field_generic(&self, m: &Message) -> bool {
        match self.fns {
            FieldAccessorFunctions::SingularHasGetSet { has, .. } => has(message_down_cast(m)),
            FieldAccessorFunctions::Optional(ref a) => {
                a.get_field(message_down_cast(m)).to_option().is_some()
            }
            FieldAccessorFunctions::Simple(ref a) => {
                a.get_field(message_down_cast(m)).is_non_zero()
            }
            FieldAccessorFunctions::Map(..) | FieldAccessorFunctions::Repeated(..) => {
                panic!("has_xxx is not implemented for repeated");
            }
        }
    }

    fn len_field_generic(&self, m: &Message) -> usize {
        match self.fns {
            FieldAccessorFunctions::Repeated(ref a) => a.get_field(message_down_cast(m)).len(),
            FieldAccessorFunctions::Map(ref a) => a.get_field(message_down_cast(m)).len(),
            FieldAccessorFunctions::Simple(..)
            | FieldAccessorFunctions::SingularHasGetSet { .. }
            | FieldAccessorFunctions::Optional(..) => {
                panic!("not a repeated field");
            }
        }
    }

    fn get_message_generic<'a>(&self, m: &'a Message) -> &'a Message {
        match self.fns {
            FieldAccessorFunctions::SingularHasGetSet {
                get_set: SingularGetSet::Message(ref get),
                ..
            } => get.get_message(message_down_cast(m)),
            FieldAccessorFunctions::Optional(ref t) => {
                match t
                    .get_field(message_down_cast(m))
                    .to_option()
                    .expect("field unset")
                    .as_ref()
                {
                    ReflectValueRef::Message(m) => m,
                    _ => panic!("not a message"),
                }
            }
            ref fns => panic!("unknown accessor type: {:?}", fns),
        }
    }

    fn get_enum_generic(&self, m: &Message) -> &'static EnumValueDescriptor {
        match self.fns {
            FieldAccessorFunctions::SingularHasGetSet {
                get_set: SingularGetSet::Enum(ref get),
                ..
            } => get.get_enum(message_down_cast(m)),
            _ => panic!(),
        }
    }

    fn get_str_generic<'a>(&self, m: &'a Message) -> &'a str {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::String(v)) => v,
            Some(_) => panic!("wrong type"),
            None => "", // TODO: check type
        }
    }

    fn get_bytes_generic<'a>(&self, m: &'a Message) -> &'a [u8] {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::Bytes(v)) => v,
            Some(_) => panic!("wrong type"),
            None => b"", // TODO: check type
        }
    }

    fn get_u32_generic(&self, m: &Message) -> u32 {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::U32(v)) => v,
            Some(_) => panic!("wrong type"),
            None => 0, // TODO: check type
        }
    }

    fn get_u64_generic(&self, m: &Message) -> u64 {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::U64(v)) => v,
            Some(_) => panic!("wrong type"),
            None => 0, // TODO: check type
        }
    }

    fn get_i32_generic(&self, m: &Message) -> i32 {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::I32(v)) => v,
            Some(_) => panic!("wrong type"),
            None => 0, // TODO: check type
        }
    }

    fn get_i64_generic(&self, m: &Message) -> i64 {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::I64(v)) => v,
            Some(_) => panic!("wrong type"),
            None => 0, // TODO: check type
        }
    }

    fn get_f32_generic(&self, m: &Message) -> f32 {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::F32(v)) => v,
            Some(_) => panic!("wrong type"),
            None => 0.0, // TODO: check type
        }
    }

    fn get_f64_generic(&self, m: &Message) -> f64 {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::F64(v)) => v,
            Some(_) => panic!("wrong type"),
            None => 0.0, // TODO: check type
        }
    }

    fn get_bool_generic(&self, m: &Message) -> bool {
        match self.get_value_option(message_down_cast(m)) {
            Some(ReflectValueRef::Bool(v)) => v,
            Some(_) => panic!("wrong type"),
            None => false, // TODO: check type
        }
    }

    fn get_reflect<'a>(&self, m: &'a Message) -> ReflectFieldRef<'a> {
        match self.fns {
            FieldAccessorFunctions::Repeated(ref accessor2) => {
                ReflectFieldRef::Repeated(accessor2.get_field(message_down_cast(m)))
            }
            FieldAccessorFunctions::Map(ref accessor2) => {
                ReflectFieldRef::Map(accessor2.get_field(message_down_cast(m)))
            }
            FieldAccessorFunctions::Optional(ref accessor2) => ReflectFieldRef::Optional(
                accessor2
                    .get_field(message_down_cast(m))
                    .to_option()
                    .map(|v| v.as_ref()),
            ),
            FieldAccessorFunctions::Simple(ref accessor2) => ReflectFieldRef::Optional({
                let v = accessor2.get_field(message_down_cast(m));
                if v.is_non_zero() {
                    Some(v.as_ref())
                } else {
                    None
                }
            }),
            FieldAccessorFunctions::SingularHasGetSet {
                ref has,
                ref get_set,
            } => ReflectFieldRef::Optional(if has(message_down_cast(m)) {
                Some(get_set.get_ref(message_down_cast(m)))
            } else {
                None
            }),
        }
    }
}

// singular

fn set_panic<A, B>(_: &mut A, _: B) {
    panic!()
}

// TODO: make_singular_xxx_accessor are used only for oneof fields
// oneof codegen should be changed

pub fn make_singular_u32_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> u32,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Copy(Box::new(GetSetCopyFnsImpl {
                get: get,
                _set: set_panic,
            })),
        },
    })
}

pub fn make_singular_i32_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> i32,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Copy(Box::new(GetSetCopyFnsImpl {
                get: get,
                _set: set_panic,
            })),
        },
    })
}

pub fn make_singular_u64_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> u64,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Copy(Box::new(GetSetCopyFnsImpl {
                get: get,
                _set: set_panic,
            })),
        },
    })
}

pub fn make_singular_i64_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> i64,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Copy(Box::new(GetSetCopyFnsImpl {
                get: get,
                _set: set_panic,
            })),
        },
    })
}

pub fn make_singular_f32_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> f32,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Copy(Box::new(GetSetCopyFnsImpl {
                get: get,
                _set: set_panic,
            })),
        },
    })
}

pub fn make_singular_f64_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> f64,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Copy(Box::new(GetSetCopyFnsImpl {
                get: get,
                _set: set_panic,
            })),
        },
    })
}

pub fn make_singular_bool_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> bool,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Copy(Box::new(GetSetCopyFnsImpl {
                get: get,
                _set: set_panic,
            })),
        },
    })
}

pub fn make_singular_enum_accessor<M: Message + 'static, E: ProtobufEnum + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: fn(&M) -> E,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Enum(Box::new(GetSingularEnumImpl { get: get })),
        },
    })
}

pub fn make_singular_string_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: for<'a> fn(&'a M) -> &'a str,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::String(get, set_panic),
        },
    })
}

pub fn make_singular_bytes_accessor<M: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: for<'a> fn(&'a M) -> &'a [u8],
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Bytes(get, set_panic),
        },
    })
}

pub fn make_singular_message_accessor<M: Message + 'static, F: Message + 'static>(
    name: &'static str,
    has: fn(&M) -> bool,
    get: for<'a> fn(&'a M) -> &'a F,
) -> Box<FieldAccessor + 'static> {
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::SingularHasGetSet {
            has: has,
            get_set: SingularGetSet::Message(Box::new(GetSingularMessageImpl { get: get })),
        },
    })
}

// repeated

impl<M, V> FieldAccessor2<M, ReflectRepeated> for MessageGetMut<M, Vec<V>>
where
    M: Message + 'static,
    V: ProtobufValue + 'static,
{
    fn get_field<'a>(&self, m: &'a M) -> &'a ReflectRepeated {
        (self.get_field)(m) as &ReflectRepeated
    }

    fn mut_field<'a>(&self, m: &'a mut M) -> &'a mut ReflectRepeated {
        (self.mut_field)(m) as &mut ReflectRepeated
    }
}

pub fn make_vec_accessor<M, V>(
    name: &'static str,
    get_vec: for<'a> fn(&'a M) -> &'a Vec<V::Value>,
    mut_vec: for<'a> fn(&'a mut M) -> &'a mut Vec<V::Value>,
) -> Box<FieldAccessor + 'static>
where
    M: Message + 'static,
    V: ProtobufType + 'static,
{
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::Repeated(Box::new(MessageGetMut::<M, Vec<V::Value>> {
            get_field: get_vec,
            mut_field: mut_vec,
        })),
    })
}

impl<M, V> FieldAccessor2<M, ReflectRepeated> for MessageGetMut<M, RepeatedField<V>>
where
    M: Message + 'static,
    V: ProtobufValue + 'static,
{
    fn get_field<'a>(&self, m: &'a M) -> &'a ReflectRepeated {
        (self.get_field)(m) as &ReflectRepeated
    }

    fn mut_field<'a>(&self, m: &'a mut M) -> &'a mut ReflectRepeated {
        (self.mut_field)(m) as &mut ReflectRepeated
    }
}

pub fn make_repeated_field_accessor<M, V>(
    name: &'static str,
    get_vec: for<'a> fn(&'a M) -> &'a RepeatedField<V::Value>,
    mut_vec: for<'a> fn(&'a mut M) -> &'a mut RepeatedField<V::Value>,
) -> Box<FieldAccessor + 'static>
where
    M: Message + 'static,
    V: ProtobufType + 'static,
{
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::Repeated(Box::new(MessageGetMut::<
            M,
            RepeatedField<V::Value>,
        > {
            get_field: get_vec,
            mut_field: mut_vec,
        })),
    })
}

impl<M, V> FieldAccessor2<M, ReflectOptional> for MessageGetMut<M, Option<V>>
where
    M: Message + 'static,
    V: ProtobufValue + Clone + 'static,
{
    fn get_field<'a>(&self, m: &'a M) -> &'a ReflectOptional {
        (self.get_field)(m) as &ReflectOptional
    }

    fn mut_field<'a>(&self, m: &'a mut M) -> &'a mut ReflectOptional {
        (self.mut_field)(m) as &mut ReflectOptional
    }
}

pub fn make_option_accessor<M, V>(
    name: &'static str,
    get_field: for<'a> fn(&'a M) -> &'a Option<V::Value>,
    mut_field: for<'a> fn(&'a mut M) -> &'a mut Option<V::Value>,
) -> Box<FieldAccessor + 'static>
where
    M: Message + 'static,
    V: ProtobufType + 'static,
{
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::Optional(Box::new(MessageGetMut::<M, Option<V::Value>> {
            get_field: get_field,
            mut_field: mut_field,
        })),
    })
}

impl<M, V> FieldAccessor2<M, ReflectOptional> for MessageGetMut<M, SingularField<V>>
where
    M: Message + 'static,
    V: ProtobufValue + Clone + 'static,
{
    fn get_field<'a>(&self, m: &'a M) -> &'a ReflectOptional {
        (self.get_field)(m) as &ReflectOptional
    }

    fn mut_field<'a>(&self, m: &'a mut M) -> &'a mut ReflectOptional {
        (self.mut_field)(m) as &mut ReflectOptional
    }
}

pub fn make_singular_field_accessor<M, V>(
    name: &'static str,
    get_field: for<'a> fn(&'a M) -> &'a SingularField<V::Value>,
    mut_field: for<'a> fn(&'a mut M) -> &'a mut SingularField<V::Value>,
) -> Box<FieldAccessor + 'static>
where
    M: Message + 'static,
    V: ProtobufType + 'static,
{
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::Optional(Box::new(MessageGetMut::<
            M,
            SingularField<V::Value>,
        > {
            get_field: get_field,
            mut_field: mut_field,
        })),
    })
}

impl<M, V> FieldAccessor2<M, ReflectOptional> for MessageGetMut<M, SingularPtrField<V>>
where
    M: Message + 'static,
    V: ProtobufValue + Clone + 'static,
{
    fn get_field<'a>(&self, m: &'a M) -> &'a ReflectOptional {
        (self.get_field)(m) as &ReflectOptional
    }

    fn mut_field<'a>(&self, m: &'a mut M) -> &'a mut ReflectOptional {
        (self.mut_field)(m) as &mut ReflectOptional
    }
}

pub fn make_singular_ptr_field_accessor<M, V>(
    name: &'static str,
    get_field: for<'a> fn(&'a M) -> &'a SingularPtrField<V::Value>,
    mut_field: for<'a> fn(&'a mut M) -> &'a mut SingularPtrField<V::Value>,
) -> Box<FieldAccessor + 'static>
where
    M: Message + 'static,
    V: ProtobufType + 'static,
{
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::Optional(Box::new(MessageGetMut::<
            M,
            SingularPtrField<V::Value>,
        > {
            get_field: get_field,
            mut_field: mut_field,
        })),
    })
}

impl<M, V> FieldAccessor2<M, ProtobufValue> for MessageGetMut<M, V>
where
    M: Message + 'static,
    V: ProtobufValue + Clone + 'static,
{
    fn get_field<'a>(&self, m: &'a M) -> &'a ProtobufValue {
        (self.get_field)(m) as &ProtobufValue
    }

    fn mut_field<'a>(&self, m: &'a mut M) -> &'a mut ProtobufValue {
        (self.mut_field)(m) as &mut ProtobufValue
    }
}

pub fn make_simple_field_accessor<M, V>(
    name: &'static str,
    get_field: for<'a> fn(&'a M) -> &'a V::Value,
    mut_field: for<'a> fn(&'a mut M) -> &'a mut V::Value,
) -> Box<FieldAccessor + 'static>
where
    M: Message + 'static,
    V: ProtobufType + 'static,
{
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::Simple(Box::new(MessageGetMut::<M, V::Value> {
            get_field: get_field,
            mut_field: mut_field,
        })),
    })
}

impl<M, K, V> FieldAccessor2<M, ReflectMap> for MessageGetMut<M, HashMap<K, V>>
where
    M: Message + 'static,
    K: ProtobufValue + 'static,
    V: ProtobufValue + 'static,
    K: Hash + Eq,
{
    fn get_field<'a>(&self, m: &'a M) -> &'a ReflectMap {
        (self.get_field)(m) as &ReflectMap
    }

    fn mut_field<'a>(&self, m: &'a mut M) -> &'a mut ReflectMap {
        (self.mut_field)(m) as &mut ReflectMap
    }
}

pub fn make_map_accessor<M, K, V>(
    name: &'static str,
    get_field: for<'a> fn(&'a M) -> &'a HashMap<K::Value, V::Value>,
    mut_field: for<'a> fn(&'a mut M) -> &'a mut HashMap<K::Value, V::Value>,
) -> Box<FieldAccessor + 'static>
where
    M: Message + 'static,
    K: ProtobufType + 'static,
    V: ProtobufType + 'static,
    <K as ProtobufType>::Value: Hash + Eq,
{
    Box::new(FieldAccessorImpl {
        name: name,
        fns: FieldAccessorFunctions::Map(Box::new(
            MessageGetMut::<M, HashMap<K::Value, V::Value>> {
                get_field: get_field,
                mut_field: mut_field,
            },
        )),
    })
}
