use crate::descriptor::FieldDescriptorProto;
use crate::descriptor::FieldDescriptorProto_Label;
use crate::json::json_name;
use crate::message::Message;
use crate::reflect::acc::Accessor;
use crate::reflect::acc::FieldAccessor;
use crate::reflect::map::ReflectMap;
use crate::reflect::repeated::ReflectRepeated;
use crate::reflect::EnumValueDescriptor;
use crate::reflect::ReflectValueRef;

/// Reference to a value stored in a field, optional, repeated or map.
// TODO: implement Eq
pub enum ReflectFieldRef<'a> {
    /// Singular field, optional or required in proto3 and just plain field in proto3
    Optional(Option<ReflectValueRef<'a>>),
    /// Repeated field
    Repeated(&'a dyn ReflectRepeated),
    /// Map field
    Map(&'a dyn ReflectMap),
}

/// Field descriptor.
///
/// Can be used for runtime reflection.
pub struct FieldDescriptor {
    proto: &'static FieldDescriptorProto,
    accessor: FieldAccessor,
    json_name: String,
}

impl FieldDescriptor {
    pub(crate) fn new(
        accessor: FieldAccessor,
        proto: &'static FieldDescriptorProto,
    ) -> FieldDescriptor {
        assert_eq!(proto.get_name(), accessor.name);
        let json_name = if !proto.get_json_name().is_empty() {
            proto.get_json_name().to_string()
        } else {
            json_name(proto.get_name())
        };
        FieldDescriptor {
            proto,
            accessor,
            // probably could be lazy-init
            json_name,
        }
    }

    /// Get `.proto` description of field
    pub fn proto(&self) -> &'static FieldDescriptorProto {
        self.proto
    }

    /// Field name as specified in `.proto` file
    pub fn name(&self) -> &'static str {
        self.proto.get_name()
    }

    /// JSON field name.
    ///
    /// Can be different from `.proto` field name.
    ///
    /// See [JSON mapping][json] for details.
    ///
    /// [json]: https://developers.google.com/protocol-buffers/docs/proto3#json
    pub fn json_name(&self) -> &str {
        &self.json_name
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
    pub fn has_field(&self, m: &dyn Message) -> bool {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.has_field_generic(m),
        }
    }

    /// Return length of repeated field.
    ///
    /// For singular field return `1` if field is set and `0` otherwise.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type.
    pub fn len_field(&self, m: &dyn Message) -> usize {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.len_field_generic(m),
        }
    }

    /// Get message field or default instance if field is unset.
    ///
    /// # Panics
    /// If this field belongs to a different message type or
    /// field type is not message.
    pub fn get_message<'a>(&self, m: &'a dyn Message) -> &'a dyn Message {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_message_generic(m),
        }
    }

    /// Get `enum` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `enum`.
    pub fn get_enum(&self, m: &dyn Message) -> &'static EnumValueDescriptor {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_enum_generic(m),
        }
    }

    /// Get `string` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `string`.
    pub fn get_str<'a>(&self, m: &'a dyn Message) -> &'a str {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_str_generic(m),
        }
    }

    /// Get `bytes` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `bytes`.
    pub fn get_bytes<'a>(&self, m: &'a dyn Message) -> &'a [u8] {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_bytes_generic(m),
        }
    }

    /// Get `u32` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `u32`.
    pub fn get_u32(&self, m: &dyn Message) -> u32 {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_u32_generic(m),
        }
    }

    /// Get `u64` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `u64`.
    pub fn get_u64(&self, m: &dyn Message) -> u64 {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_u64_generic(m),
        }
    }

    /// Get `i32` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `i32`.
    pub fn get_i32(&self, m: &dyn Message) -> i32 {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_i32_generic(m),
        }
    }

    /// Get `i64` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `i64`.
    pub fn get_i64(&self, m: &dyn Message) -> i64 {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_i64_generic(m),
        }
    }

    /// Get `bool` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type or
    /// field type is not singular `bool`.
    pub fn get_bool(&self, m: &dyn Message) -> bool {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_bool_generic(m),
        }
    }

    /// Get `float` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type or
    /// field type is not singular `float`.
    pub fn get_f32(&self, m: &dyn Message) -> f32 {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_f32_generic(m),
        }
    }

    /// Get `double` field.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type
    /// or field type is not singular `double`.
    pub fn get_f64(&self, m: &dyn Message) -> f64 {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_f64_generic(m),
        }
    }

    /// Get field of any type.
    ///
    /// # Panics
    ///
    /// If this field belongs to a different message type.
    pub fn get_reflect<'a>(&self, m: &'a dyn Message) -> ReflectFieldRef<'a> {
        match &self.accessor.accessor {
            Accessor::V1(a) => a.get_reflect(m),
        }
    }
}
