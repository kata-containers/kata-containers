use std::collections::HashMap;

use crate::descriptor::EnumDescriptorProto;
use crate::descriptor::EnumValueDescriptorProto;
use crate::descriptor::FileDescriptorProto;
use crate::descriptorx::find_enum_by_rust_name;
use crate::reflect::find_message_or_enum::find_message_or_enum;
use crate::reflect::find_message_or_enum::MessageOrEnum;
use crate::ProtobufEnum;

/// Description for enum variant.
///
/// Used in reflection.
#[derive(Clone, Debug)]
pub struct EnumValueDescriptor {
    proto: &'static EnumValueDescriptorProto,
}

impl Copy for EnumValueDescriptor {}

impl EnumValueDescriptor {
    /// Name of enum variant as specified in proto file
    pub fn name(&self) -> &'static str {
        self.proto.get_name()
    }

    /// `i32` value of the enum variant
    pub fn value(&self) -> i32 {
        self.proto.get_number()
    }
}

/// Dynamic representation of enum type.
///
/// Can be used in reflective operations.
pub struct EnumDescriptor {
    proto: &'static EnumDescriptorProto,
    values: Vec<EnumValueDescriptor>,

    index_by_name: HashMap<String, usize>,
    index_by_number: HashMap<i32, usize>,
}

impl EnumDescriptor {
    /// Enum name as given in `.proto` file
    pub fn name(&self) -> &'static str {
        self.proto.get_name()
    }

    /// `EnumDescriptor` for enum type
    pub fn for_type<E: ProtobufEnum>() -> &'static EnumDescriptor {
        E::enum_descriptor_static()
    }

    /// Create new enum descriptor.
    ///
    /// This function is called by generated code, and should not be called manually.
    #[deprecated(
        since = "2.12",
        note = "Please regenerate .rs files from .proto files to use newer APIs"
    )]
    pub fn new(rust_name: &'static str, file: &'static FileDescriptorProto) -> EnumDescriptor {
        let proto = find_enum_by_rust_name(file, rust_name);
        let mut index_by_name = HashMap::new();
        let mut index_by_number = HashMap::new();
        for (i, v) in proto.en.get_value().iter().enumerate() {
            index_by_number.insert(v.get_number(), i);
            index_by_name.insert(v.get_name().to_string(), i);
        }
        EnumDescriptor {
            proto: proto.en,
            values: proto
                .en
                .get_value()
                .iter()
                .map(|v| EnumValueDescriptor { proto: v })
                .collect(),
            index_by_name: index_by_name,
            index_by_number: index_by_number,
        }
    }

    /// Create new enum descriptor.
    ///
    /// This function is called by generated code, and should not be called manually.
    pub fn new_pb_name<E>(
        name_in_file: &'static str,
        file: &'static FileDescriptorProto,
    ) -> EnumDescriptor
    where
        E: ProtobufEnum,
    {
        let (_path_to_package, proto) = match find_message_or_enum(file, name_in_file) {
            (path_to_package, MessageOrEnum::Enum(e)) => (path_to_package, e),
            (_, MessageOrEnum::Message(_)) => panic!("not an enum"),
        };

        let mut index_by_name = HashMap::new();
        let mut index_by_number = HashMap::new();
        for (i, v) in proto.get_value().iter().enumerate() {
            index_by_number.insert(v.get_number(), i);
            index_by_name.insert(v.get_name().to_string(), i);
        }
        EnumDescriptor {
            proto,
            values: proto
                .get_value()
                .iter()
                .map(|v| EnumValueDescriptor { proto: v })
                .collect(),
            index_by_name: index_by_name,
            index_by_number: index_by_number,
        }
    }

    /// Find enum value by name
    pub fn value_by_name<'a>(&'a self, name: &str) -> &'a EnumValueDescriptor {
        // TODO: clone is weird
        let &index = self.index_by_name.get(&name.to_string()).unwrap();
        &self.values[index]
    }

    /// Find enum value by number
    pub fn value_by_number<'a>(&'a self, number: i32) -> &'a EnumValueDescriptor {
        let &index = self.index_by_number.get(&number).unwrap();
        &self.values[index]
    }
}
