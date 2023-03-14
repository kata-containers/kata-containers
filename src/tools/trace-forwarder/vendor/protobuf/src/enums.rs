use crate::reflect::EnumDescriptor;
use crate::reflect::EnumValueDescriptor;

/// Trait implemented by all protobuf enum types.
pub trait ProtobufEnum: Eq + Sized + Copy + 'static {
    /// Get enum `i32` value.
    fn value(&self) -> i32;

    /// Try to create an enum from `i32` value.
    /// Return `None` if value is unknown.
    fn from_i32(v: i32) -> Option<Self>;

    /// Get all enum values for enum type.
    fn values() -> &'static [Self] {
        panic!();
    }

    /// Get enum value descriptor.
    fn descriptor(&self) -> &'static EnumValueDescriptor {
        self.enum_descriptor().value_by_number(self.value())
    }

    /// Get enum descriptor.
    fn enum_descriptor(&self) -> &'static EnumDescriptor {
        Self::enum_descriptor_static()
    }

    /// Get enum descriptor by type.
    fn enum_descriptor_static() -> &'static EnumDescriptor {
        panic!();
    }
}
