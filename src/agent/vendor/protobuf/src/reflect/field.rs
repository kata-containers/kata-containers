use descriptor::{FieldDescriptorProto, FieldDescriptorProto_Label};
use reflect::accessor::FieldAccessor;
use reflect::map::ReflectMap;
use reflect::repeated::ReflectRepeated;
use reflect::{EnumValueDescriptor, ReflectValueRef};
use Message;

/// Reference to a value stored in a field, optional, repeated or map.
// TODO: implement Eq
pub enum ReflectFieldRef<'a> {
    /// Singular field, optional or required in proto3 and just plain field in proto3
    Optional(Option<ReflectValueRef<'a>>),
    /// Repeated field
    Repeated(&'a ReflectRepeated),
    /// Map field
    Map(&'a ReflectMap),
}

/// Field descriptor.
///
/// Can be used for runtime reflection.
pub struct FieldDescriptor {
    proto: &'static FieldDescriptorProto,
    accessor: Box<FieldAccessor + 'static>,
}

impl FieldDescriptor {
    pub(crate) fn new(
        accessor: Box<FieldAccessor + 'static>,
        proto: &'static FieldDescriptorProto,
    ) -> FieldDescriptor {
        assert_eq!(proto.get_name(), accessor.name_generic());
        FieldDescriptor { proto, accessor }
    }

    /// Get `.proto` description of field
    pub fn proto(&self) -> &'static FieldDescriptorProto {
        self.proto
    }

    /// Field name as specified in `.proto` file
    pub fn name(&self) -> &'static str {
        self.proto.get_name()
    }

    /// If this field repeated?
    pub fn is_repeated(&self) -> bool {
        self.proto.get_label() == FieldDescriptorProto_Label::LABEL_REPEATED
    }

    /// Check if field is set in given message.
    ///
    /// For repeated field or map field return `true` if
    /// collection is not empty.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type.
    pub fn has_field(&self, m: &Message) -> bool {
        self.accessor.has_field_generic(m)
    }

    /// Return length of repeated field.
    ///
    /// For singualar field return `1` if field is set and `0` otherwise.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type.
    pub fn len_field(&self, m: &dyn Message) -> usize {
        self.accessor.len_field_generic(m)
    }

    /// Get message field or default instance if field is unset.
    ///
    /// # Panics
    /// If this field belongs to a different message type or
    /// field type is not message.
    pub fn get_message<'a>(&self, m: &'a dyn Message) -> &'a dyn Message {
        self.accessor.get_message_generic(m)
    }

    /// Get `enum` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `enum`.
    pub fn get_enum(&self, m: &dyn Message) -> &'static EnumValueDescriptor {
        self.accessor.get_enum_generic(m)
    }

    /// Get `string` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `string`.
    pub fn get_str<'a>(&self, m: &'a dyn Message) -> &'a str {
        self.accessor.get_str_generic(m)
    }

    /// Get `bytes` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `bytes`.
    pub fn get_bytes<'a>(&self, m: &'a dyn Message) -> &'a [u8] {
        self.accessor.get_bytes_generic(m)
    }

    /// Get `u32` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `u32`.
    pub fn get_u32(&self, m: &dyn Message) -> u32 {
        self.accessor.get_u32_generic(m)
    }

    /// Get `u64` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `u64`.
    pub fn get_u64(&self, m: &dyn Message) -> u64 {
        self.accessor.get_u64_generic(m)
    }

    /// Get `i32` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `i32`.
    pub fn get_i32(&self, m: &dyn Message) -> i32 {
        self.accessor.get_i32_generic(m)
    }

    /// Get `i64` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `i64`.
    pub fn get_i64(&self, m: &dyn Message) -> i64 {
        self.accessor.get_i64_generic(m)
    }

    /// Get `bool` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type or
    /// field type is not singular `bool`.
    pub fn get_bool(&self, m: &dyn Message) -> bool {
        self.accessor.get_bool_generic(m)
    }

    /// Get `float` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type or
    /// field type is not singular `float`.
    pub fn get_f32(&self, m: &dyn Message) -> f32 {
        self.accessor.get_f32_generic(m)
    }

    /// Get `double` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `double`.
    pub fn get_f64(&self, m: &dyn Message) -> f64 {
        self.accessor.get_f64_generic(m)
    }

    /// Get field of any type.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type.
    pub fn get_reflect<'a>(&self, m: &'a dyn Message) -> ReflectFieldRef<'a> {
        self.accessor.get_reflect(m)
    }
}
