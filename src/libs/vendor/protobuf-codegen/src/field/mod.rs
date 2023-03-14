use std::marker;

use float;
use inside::protobuf_crate_path;
use message::RustTypeMessage;
use oneof::OneofField;
use protobuf::descriptor::*;
use protobuf::rt;
use protobuf::rust;
use protobuf::text_format;
use protobuf::wire_format;
use protobuf_name::ProtobufAbsolutePath;
use rust_name::RustIdent;
use rust_name::RustIdentWithPath;
use scope::FieldWithContext;
use scope::MessageOrEnumWithScope;
use scope::RootScope;
use scope::WithScope;
use syntax::Syntax;

use super::code_writer::CodeWriter;
use super::customize::customize_from_rustproto_for_field;
use super::customize::Customize;
use super::enums::*;
use super::rust_types_values::*;

fn type_is_copy(field_type: FieldDescriptorProto_Type) -> bool {
    match field_type {
        FieldDescriptorProto_Type::TYPE_MESSAGE
        | FieldDescriptorProto_Type::TYPE_STRING
        | FieldDescriptorProto_Type::TYPE_BYTES => false,
        _ => true,
    }
}

trait FieldDescriptorProtoTypeExt {
    fn read(&self, is: &str, primitive_type_variant: PrimitiveTypeVariant) -> String;
    fn is_s_varint(&self) -> bool;
}

impl FieldDescriptorProtoTypeExt for FieldDescriptorProto_Type {
    fn read(&self, is: &str, primitive_type_variant: PrimitiveTypeVariant) -> String {
        match primitive_type_variant {
            PrimitiveTypeVariant::Default => format!("{}.read_{}()", is, protobuf_name(*self)),
            PrimitiveTypeVariant::Carllerche => {
                let protobuf_name = match self {
                    &FieldDescriptorProto_Type::TYPE_STRING => "chars",
                    _ => protobuf_name(*self),
                };
                format!("{}.read_carllerche_{}()", is, protobuf_name)
            }
        }
    }

    /// True if self is signed integer with zigzag encoding
    fn is_s_varint(&self) -> bool {
        match *self {
            FieldDescriptorProto_Type::TYPE_SINT32 | FieldDescriptorProto_Type::TYPE_SINT64 => true,
            _ => false,
        }
    }
}

fn field_type_wire_type(field_type: FieldDescriptorProto_Type) -> wire_format::WireType {
    use protobuf::wire_format::*;
    match field_type {
        FieldDescriptorProto_Type::TYPE_INT32 => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_INT64 => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_UINT32 => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_UINT64 => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_SINT32 => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_SINT64 => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_BOOL => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_ENUM => WireTypeVarint,
        FieldDescriptorProto_Type::TYPE_FIXED32 => WireTypeFixed32,
        FieldDescriptorProto_Type::TYPE_FIXED64 => WireTypeFixed64,
        FieldDescriptorProto_Type::TYPE_SFIXED32 => WireTypeFixed32,
        FieldDescriptorProto_Type::TYPE_SFIXED64 => WireTypeFixed64,
        FieldDescriptorProto_Type::TYPE_FLOAT => WireTypeFixed32,
        FieldDescriptorProto_Type::TYPE_DOUBLE => WireTypeFixed64,
        FieldDescriptorProto_Type::TYPE_STRING => WireTypeLengthDelimited,
        FieldDescriptorProto_Type::TYPE_BYTES => WireTypeLengthDelimited,
        FieldDescriptorProto_Type::TYPE_MESSAGE => WireTypeLengthDelimited,
        FieldDescriptorProto_Type::TYPE_GROUP => WireTypeLengthDelimited, // not true
    }
}

fn type_protobuf_name(field_type: FieldDescriptorProto_Type) -> &'static str {
    match field_type {
        FieldDescriptorProto_Type::TYPE_INT32 => "int32",
        FieldDescriptorProto_Type::TYPE_INT64 => "int64",
        FieldDescriptorProto_Type::TYPE_UINT32 => "uint32",
        FieldDescriptorProto_Type::TYPE_UINT64 => "uint64",
        FieldDescriptorProto_Type::TYPE_SINT32 => "sint32",
        FieldDescriptorProto_Type::TYPE_SINT64 => "sint64",
        FieldDescriptorProto_Type::TYPE_BOOL => "bool",
        FieldDescriptorProto_Type::TYPE_FIXED32 => "fixed32",
        FieldDescriptorProto_Type::TYPE_FIXED64 => "fixed64",
        FieldDescriptorProto_Type::TYPE_SFIXED32 => "sfixed32",
        FieldDescriptorProto_Type::TYPE_SFIXED64 => "sfixed64",
        FieldDescriptorProto_Type::TYPE_FLOAT => "float",
        FieldDescriptorProto_Type::TYPE_DOUBLE => "double",
        FieldDescriptorProto_Type::TYPE_STRING => "string",
        FieldDescriptorProto_Type::TYPE_BYTES => "bytes",
        FieldDescriptorProto_Type::TYPE_ENUM
        | FieldDescriptorProto_Type::TYPE_MESSAGE
        | FieldDescriptorProto_Type::TYPE_GROUP => panic!(),
    }
}

fn field_type_protobuf_name<'a>(field: &'a FieldDescriptorProto) -> &'a str {
    if field.has_type_name() {
        field.get_type_name()
    } else {
        type_protobuf_name(field.get_field_type())
    }
}

// size of value for type, None if variable
fn field_type_size(field_type: FieldDescriptorProto_Type) -> Option<u32> {
    match field_type {
        FieldDescriptorProto_Type::TYPE_BOOL => Some(1),
        t if field_type_wire_type(t) == wire_format::WireTypeFixed32 => Some(4),
        t if field_type_wire_type(t) == wire_format::WireTypeFixed64 => Some(8),
        _ => None,
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum SingularFieldFlag {
    // proto2 or proto3 message
    WithFlag { required: bool },
    // proto3
    WithoutFlag,
}

impl SingularFieldFlag {
    pub fn is_required(&self) -> bool {
        match *self {
            SingularFieldFlag::WithFlag { required, .. } => required,
            SingularFieldFlag::WithoutFlag => false,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SingularField<'a> {
    pub flag: SingularFieldFlag,
    pub elem: FieldElem<'a>,
}

impl<'a> SingularField<'a> {
    fn rust_storage_type(&self) -> RustType {
        match self.flag {
            SingularFieldFlag::WithFlag { .. } => match self.elem.proto_type() {
                FieldDescriptorProto_Type::TYPE_MESSAGE => {
                    RustType::SingularPtrField(Box::new(self.elem.rust_storage_type()))
                }
                FieldDescriptorProto_Type::TYPE_STRING | FieldDescriptorProto_Type::TYPE_BYTES
                    if self.elem.primitive_type_variant() == PrimitiveTypeVariant::Default =>
                {
                    RustType::SingularField(Box::new(self.elem.rust_storage_type()))
                }
                _ => RustType::Option(Box::new(self.elem.rust_storage_type())),
            },
            SingularFieldFlag::WithoutFlag => self.elem.rust_storage_type(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RepeatedField<'a> {
    pub elem: FieldElem<'a>,
    pub packed: bool,
}

impl<'a> RepeatedField<'a> {
    fn rust_type(&self) -> RustType {
        if !self.elem.is_copy()
            && self.elem.primitive_type_variant() != PrimitiveTypeVariant::Carllerche
        {
            RustType::RepeatedField(Box::new(self.elem.rust_storage_type()))
        } else {
            RustType::Vec(Box::new(self.elem.rust_storage_type()))
        }
    }
}

#[derive(Clone)]
pub struct MapField<'a> {
    _name: String,
    key: FieldElem<'a>,
    value: FieldElem<'a>,
}

#[derive(Clone)]
pub(crate) enum FieldKind<'a> {
    // optional or required
    Singular(SingularField<'a>),
    // repeated except map
    Repeated(RepeatedField<'a>),
    // map
    Map(MapField<'a>),
    // part of oneof
    Oneof(OneofField<'a>),
}

impl<'a> FieldKind<'a> {
    fn elem(&self) -> &FieldElem {
        match self {
            &FieldKind::Singular(ref s) => &s.elem,
            &FieldKind::Repeated(ref r) => &r.elem,
            &FieldKind::Oneof(ref o) => &o.elem,
            &FieldKind::Map(..) => {
                panic!("no single elem type for map field");
            }
        }
    }

    fn primitive_type_variant(&self) -> PrimitiveTypeVariant {
        self.elem().primitive_type_variant()
    }
}

// Representation of map entry: key type and value type
#[derive(Clone, Debug)]
pub struct EntryKeyValue<'a>(FieldElem<'a>, FieldElem<'a>);

#[derive(Clone, Debug)]
pub(crate) enum FieldElem<'a> {
    Primitive(FieldDescriptorProto_Type, PrimitiveTypeVariant),
    // name, file name, entry
    Message(
        String,
        String,
        Option<Box<EntryKeyValue<'a>>>,
        marker::PhantomData<&'a ()>,
    ),
    // name, file name, default value
    Enum(String, String, RustIdent),
    Group,
}

impl<'a> FieldElem<'a> {
    fn proto_type(&self) -> FieldDescriptorProto_Type {
        match *self {
            FieldElem::Primitive(t, ..) => t,
            FieldElem::Group => FieldDescriptorProto_Type::TYPE_GROUP,
            FieldElem::Message(..) => FieldDescriptorProto_Type::TYPE_MESSAGE,
            FieldElem::Enum(..) => FieldDescriptorProto_Type::TYPE_ENUM,
        }
    }

    fn is_copy(&self) -> bool {
        type_is_copy(self.proto_type())
    }

    pub fn rust_storage_type(&self) -> RustType {
        match *self {
            FieldElem::Primitive(t, PrimitiveTypeVariant::Default) => rust_name(t),
            FieldElem::Primitive(
                FieldDescriptorProto_Type::TYPE_STRING,
                PrimitiveTypeVariant::Carllerche,
            ) => RustType::Chars,
            FieldElem::Primitive(
                FieldDescriptorProto_Type::TYPE_BYTES,
                PrimitiveTypeVariant::Carllerche,
            ) => RustType::Bytes,
            FieldElem::Primitive(.., PrimitiveTypeVariant::Carllerche) => unreachable!(),
            FieldElem::Group => RustType::Group,
            FieldElem::Message(ref name, ..) => {
                RustType::Message(RustTypeMessage(RustIdentWithPath::new(name.clone())))
            }
            FieldElem::Enum(ref name, _, ref default_value) => {
                RustType::Enum(name.clone(), default_value.clone())
            }
        }
    }

    fn protobuf_type_gen(&self) -> ProtobufTypeGen {
        match *self {
            FieldElem::Primitive(t, v) => ProtobufTypeGen::Primitive(t, v),
            FieldElem::Message(ref name, ..) => ProtobufTypeGen::Message(name.clone()),
            FieldElem::Enum(ref name, ..) => ProtobufTypeGen::Enum(name.clone()),
            FieldElem::Group => unreachable!(),
        }
    }

    /// implementation of ProtobufType trait
    fn lib_protobuf_type(&self, customize: &Customize) -> String {
        self.protobuf_type_gen().rust_type(customize)
    }

    fn primitive_type_variant(&self) -> PrimitiveTypeVariant {
        match self {
            &FieldElem::Primitive(_, v) => v,
            _ => PrimitiveTypeVariant::Default,
        }
    }
}

fn field_elem<'a>(
    field: &FieldWithContext,
    root_scope: &'a RootScope<'a>,
    parse_map: bool,
    customize: &Customize,
) -> (FieldElem<'a>, Option<EnumValueGen>) {
    if field.field.get_field_type() == FieldDescriptorProto_Type::TYPE_GROUP {
        (FieldElem::Group, None)
    } else if field.field.has_type_name() {
        let message_or_enum = root_scope
            .find_message_or_enum(&ProtobufAbsolutePath::from(field.field.get_type_name()));
        let file_name = message_or_enum
            .get_scope()
            .file_scope
            .file_descriptor
            .get_name()
            .to_owned();
        let rust_relative_name = type_name_to_rust_relative(
            &ProtobufAbsolutePath::from(field.field.get_type_name()),
            field.message.get_scope().file_scope.file_descriptor,
            false,
            root_scope,
            customize,
        );
        match (field.field.get_field_type(), message_or_enum) {
            (
                FieldDescriptorProto_Type::TYPE_MESSAGE,
                MessageOrEnumWithScope::Message(message_with_scope),
            ) => {
                let entry_key_value = if let (true, Some((key, value))) =
                    (parse_map, message_with_scope.map_entry())
                {
                    Some(Box::new(EntryKeyValue(
                        field_elem(&key, root_scope, false, customize).0,
                        field_elem(&value, root_scope, false, customize).0,
                    )))
                } else {
                    None
                };
                (
                    FieldElem::Message(
                        rust_relative_name,
                        file_name,
                        entry_key_value,
                        marker::PhantomData,
                    ),
                    None,
                )
            }
            (
                FieldDescriptorProto_Type::TYPE_ENUM,
                MessageOrEnumWithScope::Enum(enum_with_scope),
            ) => {
                let e = EnumGen::new(
                    &enum_with_scope,
                    field.message.get_scope().get_file_descriptor(),
                    customize,
                    root_scope,
                );
                let ev = if field.field.has_default_value() {
                    e.value_by_name(field.field.get_default_value()).clone()
                } else {
                    e.values_unique().into_iter().next().unwrap()
                };
                (
                    FieldElem::Enum(
                        rust_relative_name,
                        file_name,
                        RustIdent::from(enum_with_scope.values()[0].rust_name().to_owned()),
                    ),
                    Some(ev),
                )
            }
            _ => panic!("unknown named type: {:?}", field.field.get_field_type()),
        }
    } else if field.field.has_field_type() {
        let carllerche_for_bytes = customize.carllerche_bytes_for_bytes.unwrap_or(false);
        let carllerche_for_string = customize.carllerche_bytes_for_string.unwrap_or(false);

        let elem = match field.field.get_field_type() {
            FieldDescriptorProto_Type::TYPE_STRING if carllerche_for_string => {
                FieldElem::Primitive(
                    FieldDescriptorProto_Type::TYPE_STRING,
                    PrimitiveTypeVariant::Carllerche,
                )
            }
            FieldDescriptorProto_Type::TYPE_BYTES if carllerche_for_bytes => FieldElem::Primitive(
                FieldDescriptorProto_Type::TYPE_BYTES,
                PrimitiveTypeVariant::Carllerche,
            ),
            t => FieldElem::Primitive(t, PrimitiveTypeVariant::Default),
        };

        (elem, None)
    } else {
        panic!(
            "neither type_name, nor field_type specified for field: {}",
            field.field.get_name()
        );
    }
}

pub enum AccessorStyle {
    Lambda,
    HasGet,
}

pub struct AccessorFn {
    name: String,
    type_params: Vec<String>,
    pub style: AccessorStyle,
}

impl AccessorFn {
    pub fn sig(&self) -> String {
        let mut s = self.name.clone();
        s.push_str("::<_");
        for p in &self.type_params {
            s.push_str(", ");
            s.push_str(&p);
        }
        s.push_str(">");
        s
    }
}

#[derive(Clone)]
pub(crate) struct FieldGen<'a> {
    _root_scope: &'a RootScope<'a>,
    syntax: Syntax,
    pub proto_field: FieldWithContext<'a>,
    // field name in generated code
    pub rust_name: RustIdent,
    pub proto_type: FieldDescriptorProto_Type,
    wire_type: wire_format::WireType,
    enum_default_value: Option<EnumValueGen>,
    pub kind: FieldKind<'a>,
    pub expose_field: bool,
    pub generate_accessors: bool,
    pub(crate) customize: Customize,
}

impl<'a> FieldGen<'a> {
    pub fn parse(
        field: FieldWithContext<'a>,
        root_scope: &'a RootScope<'a>,
        customize: &Customize,
    ) -> FieldGen<'a> {
        let mut customize = customize.clone();
        customize.update_with(&customize_from_rustproto_for_field(
            &field.field.get_options(),
        ));

        let (elem, enum_default_value) = field_elem(&field, root_scope, true, &customize);

        let generate_accessors = customize.generate_accessors.unwrap_or(true);

        let syntax = field.message.scope.file_scope.syntax();

        let field_may_have_custom_default_value = syntax == Syntax::PROTO2
            && field.field.get_label() != FieldDescriptorProto_Label::LABEL_REPEATED
            && field.field.get_field_type() != FieldDescriptorProto_Type::TYPE_MESSAGE;

        let default_expose_field = !field_may_have_custom_default_value;

        let expose_field = customize.expose_fields.unwrap_or(default_expose_field);

        let kind = if field.field.get_label() == FieldDescriptorProto_Label::LABEL_REPEATED {
            match (elem, true) {
                // map field
                (FieldElem::Message(name, _, Some(key_value), _), true) => {
                    FieldKind::Map(MapField {
                        _name: name,
                        key: key_value.0.clone(),
                        value: key_value.1.clone(),
                    })
                }
                // regular repeated field
                (elem, _) => FieldKind::Repeated(RepeatedField {
                    elem,
                    packed: field.field.get_options().get_packed(),
                }),
            }
        } else if let Some(oneof) = field.oneof() {
            FieldKind::Oneof(OneofField::parse(&oneof, &field, elem, root_scope))
        } else {
            let flag = if field.message.scope.file_scope.syntax() == Syntax::PROTO3
                && field.field.get_field_type() != FieldDescriptorProto_Type::TYPE_MESSAGE
            {
                SingularFieldFlag::WithoutFlag
            } else {
                SingularFieldFlag::WithFlag {
                    required: field.field.get_label() == FieldDescriptorProto_Label::LABEL_REQUIRED,
                }
            };
            FieldKind::Singular(SingularField { elem, flag })
        };

        FieldGen {
            _root_scope: root_scope,
            syntax: field.message.get_scope().file_scope.syntax(),
            rust_name: field.rust_name(),
            proto_type: field.field.get_field_type(),
            wire_type: field_type_wire_type(field.field.get_field_type()),
            enum_default_value,
            proto_field: field.clone(),
            kind,
            expose_field,
            generate_accessors,
            customize,
        }
    }

    fn tag_size(&self) -> u32 {
        rt::tag_size(self.proto_field.number())
    }

    pub fn is_oneof(&self) -> bool {
        match self.kind {
            FieldKind::Oneof(..) => true,
            _ => false,
        }
    }

    pub fn oneof(&self) -> &OneofField {
        match self.kind {
            FieldKind::Oneof(ref oneof) => &oneof,
            _ => panic!("not a oneof field: {}", self.reconstruct_def()),
        }
    }

    fn is_singular(&self) -> bool {
        match self.kind {
            FieldKind::Singular(..) => true,
            _ => false,
        }
    }

    fn is_repeated_not_map(&self) -> bool {
        match self.kind {
            FieldKind::Repeated(..) => true,
            _ => false,
        }
    }

    fn is_repeated_or_map(&self) -> bool {
        match self.kind {
            FieldKind::Repeated(..) | FieldKind::Map(..) => true,
            _ => false,
        }
    }

    fn is_repeated_packed(&self) -> bool {
        match self.kind {
            FieldKind::Repeated(RepeatedField { packed: true, .. }) => true,
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn repeated(&self) -> &RepeatedField {
        match self.kind {
            FieldKind::Repeated(ref repeated) => &repeated,
            _ => panic!("not a repeated field: {}", self.reconstruct_def()),
        }
    }

    fn singular(&self) -> &SingularField {
        match self.kind {
            FieldKind::Singular(ref singular) => &singular,
            _ => panic!("not a singular field: {}", self.reconstruct_def()),
        }
    }

    fn map(&self) -> &MapField {
        match self.kind {
            FieldKind::Map(ref map) => &map,
            _ => panic!("not a map field: {}", self.reconstruct_def()),
        }
    }

    fn variant_path(&self) -> String {
        // TODO: should reuse code from OneofVariantGen
        format!(
            "{}::{}",
            self.oneof().oneof_type_name.to_code(&self.customize),
            self.rust_name
        )
    }

    // TODO: drop it
    pub fn elem(&self) -> &FieldElem {
        match self.kind {
            FieldKind::Singular(SingularField { ref elem, .. }) => &elem,
            FieldKind::Repeated(RepeatedField { ref elem, .. }) => &elem,
            FieldKind::Oneof(OneofField { ref elem, .. }) => &elem,
            FieldKind::Map(..) => unreachable!(),
        }
    }

    // type of field in struct
    pub fn full_storage_type(&self) -> RustType {
        match self.kind {
            FieldKind::Repeated(ref repeated) => repeated.rust_type(),
            FieldKind::Map(MapField {
                ref key, ref value, ..
            }) => RustType::HashMap(
                Box::new(key.rust_storage_type()),
                Box::new(value.rust_storage_type()),
            ),
            FieldKind::Singular(ref singular) => singular.rust_storage_type(),
            FieldKind::Oneof(..) => unreachable!(),
        }
    }

    // type of `v` in `for v in field`
    fn full_storage_iter_elem_type(&self) -> RustType {
        if let FieldKind::Oneof(ref oneof) = self.kind {
            oneof.elem.rust_storage_type()
        } else {
            self.full_storage_type().iter_elem_type()
        }
    }

    // suffix `xxx` as in `os.write_xxx_no_tag(..)`
    fn os_write_fn_suffix(&self) -> &str {
        protobuf_name(self.proto_type)
    }

    // type of `v` in `os.write_xxx_no_tag(v)`
    fn os_write_fn_param_type(&self) -> RustType {
        match self.proto_type {
            FieldDescriptorProto_Type::TYPE_STRING => RustType::Ref(Box::new(RustType::Str)),
            FieldDescriptorProto_Type::TYPE_BYTES => {
                RustType::Ref(Box::new(RustType::Slice(Box::new(RustType::Int(false, 8)))))
            }
            FieldDescriptorProto_Type::TYPE_ENUM => RustType::Int(true, 32),
            t => rust_name(t),
        }
    }

    // for field `foo`, type of param of `fn set_foo(..)`
    fn set_xxx_param_type(&self) -> RustType {
        match self.kind {
            FieldKind::Singular(SingularField { ref elem, .. })
            | FieldKind::Oneof(OneofField { ref elem, .. }) => elem.rust_storage_type(),
            FieldKind::Repeated(..) | FieldKind::Map(..) => self.full_storage_type(),
        }
    }

    // for field `foo`, return type if `fn take_foo(..)`
    fn take_xxx_return_type(&self) -> RustType {
        self.set_xxx_param_type()
    }

    // for field `foo`, return type of `fn mut_foo(..)`
    fn mut_xxx_return_type(&self) -> RustType {
        RustType::Ref(Box::new(match self.kind {
            FieldKind::Singular(SingularField { ref elem, .. })
            | FieldKind::Oneof(OneofField { ref elem, .. }) => elem.rust_storage_type(),
            FieldKind::Repeated(..) | FieldKind::Map(..) => self.full_storage_type(),
        }))
    }

    // for field `foo`, return type of `fn get_foo(..)`
    fn get_xxx_return_type(&self) -> RustType {
        match self.kind {
            FieldKind::Singular(SingularField { ref elem, .. })
            | FieldKind::Oneof(OneofField { ref elem, .. }) => match elem.is_copy() {
                true => elem.rust_storage_type(),
                false => elem.rust_storage_type().ref_type(),
            },
            FieldKind::Repeated(RepeatedField { ref elem, .. }) => RustType::Ref(Box::new(
                RustType::Slice(Box::new(elem.rust_storage_type())),
            )),
            FieldKind::Map(..) => RustType::Ref(Box::new(self.full_storage_type())),
        }
    }

    // fixed size type?
    fn is_fixed(&self) -> bool {
        field_type_size(self.proto_type).is_some()
    }

    // must use zigzag encoding?
    fn is_zigzag(&self) -> bool {
        match self.proto_type {
            FieldDescriptorProto_Type::TYPE_SINT32 | FieldDescriptorProto_Type::TYPE_SINT64 => true,
            _ => false,
        }
    }

    // data is enum
    fn is_enum(&self) -> bool {
        match self.proto_type {
            FieldDescriptorProto_Type::TYPE_ENUM => true,
            _ => false,
        }
    }

    // elem data is not stored in heap
    pub fn elem_type_is_copy(&self) -> bool {
        type_is_copy(self.proto_type)
    }

    fn defaut_value_from_proto_float(&self) -> String {
        assert!(self.proto_field.field.has_default_value());

        let type_name = match self.proto_type {
            FieldDescriptorProto_Type::TYPE_FLOAT => "f32",
            FieldDescriptorProto_Type::TYPE_DOUBLE => "f64",
            _ => unreachable!(),
        };
        let proto_default = self.proto_field.field.get_default_value();

        let f = float::parse_protobuf_float(proto_default)
            .expect(&format!("failed to parse float: {:?}", proto_default));

        if f.is_nan() {
            format!("::std::{}::NAN", type_name)
        } else if f.is_infinite() {
            if f > 0.0 {
                format!("::std::{}::INFINITY", type_name)
            } else {
                format!("::std::{}::NEG_INFINITY", type_name)
            }
        } else {
            format!("{:?}{}", f, type_name)
        }
    }

    fn default_value_from_proto(&self) -> Option<String> {
        assert!(self.is_singular() || self.is_oneof());
        if self.enum_default_value.is_some() {
            Some(self.enum_default_value.as_ref().unwrap().rust_name_outer())
        } else if self.proto_field.field.has_default_value() {
            let proto_default = self.proto_field.field.get_default_value();
            Some(match self.proto_type {
                // For numeric types, contains the original text representation of the value
                FieldDescriptorProto_Type::TYPE_DOUBLE | FieldDescriptorProto_Type::TYPE_FLOAT => {
                    self.defaut_value_from_proto_float()
                }
                FieldDescriptorProto_Type::TYPE_INT32
                | FieldDescriptorProto_Type::TYPE_SINT32
                | FieldDescriptorProto_Type::TYPE_SFIXED32 => format!("{}i32", proto_default),
                FieldDescriptorProto_Type::TYPE_UINT32
                | FieldDescriptorProto_Type::TYPE_FIXED32 => format!("{}u32", proto_default),
                FieldDescriptorProto_Type::TYPE_INT64
                | FieldDescriptorProto_Type::TYPE_SINT64
                | FieldDescriptorProto_Type::TYPE_SFIXED64 => format!("{}i64", proto_default),
                FieldDescriptorProto_Type::TYPE_UINT64
                | FieldDescriptorProto_Type::TYPE_FIXED64 => format!("{}u64", proto_default),

                // For booleans, "true" or "false"
                FieldDescriptorProto_Type::TYPE_BOOL => format!("{}", proto_default),
                // For strings, contains the default text contents (not escaped in any way)
                FieldDescriptorProto_Type::TYPE_STRING => rust::quote_escape_str(proto_default),
                // For bytes, contains the C escaped value.  All bytes >= 128 are escaped
                FieldDescriptorProto_Type::TYPE_BYTES => {
                    rust::quote_escape_bytes(&text_format::unescape_string(proto_default))
                }
                // TODO: resolve outer message prefix
                FieldDescriptorProto_Type::TYPE_GROUP | FieldDescriptorProto_Type::TYPE_ENUM => {
                    unreachable!()
                }
                FieldDescriptorProto_Type::TYPE_MESSAGE => panic!(
                    "default value is not implemented for type: {:?}",
                    self.proto_type
                ),
            })
        } else {
            None
        }
    }

    fn default_value_from_proto_typed(&self) -> Option<RustValueTyped> {
        self.default_value_from_proto().map(|v| {
            let default_value_type = match self.proto_type {
                FieldDescriptorProto_Type::TYPE_STRING => RustType::Ref(Box::new(RustType::Str)),
                FieldDescriptorProto_Type::TYPE_BYTES => {
                    RustType::Ref(Box::new(RustType::Slice(Box::new(RustType::u8()))))
                }
                _ => self.full_storage_iter_elem_type(),
            };

            RustValueTyped {
                value: v,
                rust_type: default_value_type,
            }
        })
    }

    // default value to be returned from fn get_xxx
    fn get_xxx_default_value_rust(&self) -> String {
        assert!(self.is_singular() || self.is_oneof());
        self.default_value_from_proto()
            .unwrap_or_else(|| self.get_xxx_return_type().default_value(&self.customize))
    }

    // default to be assigned to field
    fn element_default_value_rust(&self) -> RustValueTyped {
        assert!(
            self.is_singular() || self.is_oneof(),
            "field is not singular: {}",
            self.reconstruct_def()
        );
        self.default_value_from_proto_typed().unwrap_or_else(|| {
            self.elem()
                .rust_storage_type()
                .default_value_typed(&self.customize)
        })
    }

    pub fn reconstruct_def(&self) -> String {
        let prefix = match (self.proto_field.field.get_label(), self.syntax) {
            (FieldDescriptorProto_Label::LABEL_REPEATED, _) => "repeated ",
            (_, Syntax::PROTO3) => "",
            (FieldDescriptorProto_Label::LABEL_OPTIONAL, _) => "optional ",
            (FieldDescriptorProto_Label::LABEL_REQUIRED, _) => "required ",
        };
        format!(
            "{}{} {} = {}",
            prefix,
            field_type_protobuf_name(&self.proto_field.field),
            self.proto_field.name(),
            self.proto_field.number()
        )
    }

    pub fn accessor_fn(&self) -> AccessorFn {
        match self.kind {
            FieldKind::Repeated(RepeatedField { ref elem, .. }) => {
                let coll = match self.full_storage_type() {
                    RustType::Vec(..) => "vec",
                    RustType::RepeatedField(..) => "repeated_field",
                    _ => unreachable!(),
                };
                let name = format!("make_{}_accessor", coll);
                AccessorFn {
                    name: name,
                    type_params: vec![elem.lib_protobuf_type(&self.customize)],
                    style: AccessorStyle::Lambda,
                }
            }
            FieldKind::Map(MapField {
                ref key, ref value, ..
            }) => AccessorFn {
                name: "make_map_accessor".to_owned(),
                type_params: vec![
                    key.lib_protobuf_type(&self.customize),
                    value.lib_protobuf_type(&self.customize),
                ],
                style: AccessorStyle::Lambda,
            },
            FieldKind::Singular(SingularField {
                ref elem,
                flag: SingularFieldFlag::WithoutFlag,
            }) => {
                if let &FieldElem::Message(ref name, ..) = elem {
                    // TODO: old style, needed because of default instance

                    AccessorFn {
                        name: "make_singular_message_accessor".to_owned(),
                        type_params: vec![name.clone()],
                        style: AccessorStyle::HasGet,
                    }
                } else {
                    AccessorFn {
                        name: "make_simple_field_accessor".to_owned(),
                        type_params: vec![elem.lib_protobuf_type(&self.customize)],
                        style: AccessorStyle::Lambda,
                    }
                }
            }
            FieldKind::Singular(SingularField {
                ref elem,
                flag: SingularFieldFlag::WithFlag { .. },
            }) => {
                let coll = match self.full_storage_type() {
                    RustType::Option(..) => "option",
                    RustType::SingularField(..) => "singular_field",
                    RustType::SingularPtrField(..) => "singular_ptr_field",
                    _ => unreachable!(),
                };
                let name = format!("make_{}_accessor", coll);
                AccessorFn {
                    name: name,
                    type_params: vec![elem.lib_protobuf_type(&self.customize)],
                    style: AccessorStyle::Lambda,
                }
            }
            FieldKind::Oneof(OneofField { ref elem, .. }) => {
                // TODO: uses old style

                let suffix = match &self.elem().rust_storage_type() {
                    t if t.is_primitive() => t.to_code(&self.customize),
                    &RustType::String | &RustType::Chars => "string".to_string(),
                    &RustType::Vec(ref t) if t.is_u8() => "bytes".to_string(),
                    &RustType::Bytes => "bytes".to_string(),
                    &RustType::Enum(..) => "enum".to_string(),
                    &RustType::Message(..) => "message".to_string(),
                    t => panic!("unexpected field type: {:?}", t),
                };

                let name = format!("make_singular_{}_accessor", suffix);

                let mut type_params = Vec::new();
                match elem {
                    &FieldElem::Message(ref name, ..) | &FieldElem::Enum(ref name, ..) => {
                        type_params.push(name.to_owned());
                    }
                    _ => (),
                }

                AccessorFn {
                    name: name,
                    type_params: type_params,
                    style: AccessorStyle::HasGet,
                }
            }
        }
    }

    pub fn write_clear(&self, w: &mut CodeWriter) {
        if self.is_oneof() {
            w.write_line(&format!(
                "self.{} = ::std::option::Option::None;",
                self.oneof().oneof_rust_field_name
            ));
        } else {
            let clear_expr = self
                .full_storage_type()
                .clear(&self.self_field(), &self.customize);
            w.write_line(&format!("{};", clear_expr));
        }
    }

    // expression that returns size of data is variable
    fn element_size(&self, var: &str, var_type: &RustType) -> String {
        assert!(!self.is_repeated_packed());

        match field_type_size(self.proto_type) {
            Some(data_size) => format!("{}", data_size + self.tag_size()),
            None => match self.proto_type {
                FieldDescriptorProto_Type::TYPE_MESSAGE => panic!("not a single-liner"),
                FieldDescriptorProto_Type::TYPE_BYTES => format!(
                    "{}::rt::bytes_size({}, &{})",
                    protobuf_crate_path(&self.customize),
                    self.proto_field.number(),
                    var
                ),
                FieldDescriptorProto_Type::TYPE_STRING => format!(
                    "{}::rt::string_size({}, &{})",
                    protobuf_crate_path(&self.customize),
                    self.proto_field.number(),
                    var
                ),
                FieldDescriptorProto_Type::TYPE_ENUM => {
                    let param_type = match var_type {
                        &RustType::Ref(ref t) => (**t).clone(),
                        t => t.clone(),
                    };
                    format!(
                        "{}::rt::enum_size({}, {})",
                        protobuf_crate_path(&self.customize),
                        self.proto_field.number(),
                        var_type.into_target(&param_type, var, &self.customize)
                    )
                }
                _ => {
                    let param_type = match var_type {
                        &RustType::Ref(ref t) => (**t).clone(),
                        t => t.clone(),
                    };
                    if self.proto_type.is_s_varint() {
                        format!(
                            "{}::rt::value_varint_zigzag_size({}, {})",
                            protobuf_crate_path(&self.customize),
                            self.proto_field.number(),
                            var_type.into_target(&param_type, var, &self.customize)
                        )
                    } else {
                        format!(
                            "{}::rt::value_size({}, {}, {}::wire_format::{:?})",
                            protobuf_crate_path(&self.customize),
                            self.proto_field.number(),
                            var_type.into_target(&param_type, var, &self.customize),
                            protobuf_crate_path(&self.customize),
                            self.wire_type,
                        )
                    }
                }
            },
        }
    }

    // output code that writes single element to stream
    pub fn write_write_element(&self, w: &mut CodeWriter, os: &str, var: &str, ty: &RustType) {
        if let FieldKind::Repeated(RepeatedField { packed: true, .. }) = self.kind {
            unreachable!();
        };

        match self.proto_type {
            FieldDescriptorProto_Type::TYPE_MESSAGE => {
                w.write_line(&format!(
                    "{}.write_tag({}, {}::wire_format::{:?})?;",
                    os,
                    self.proto_field.number(),
                    protobuf_crate_path(&self.customize),
                    wire_format::WireTypeLengthDelimited
                ));
                w.write_line(&format!(
                    "{}.write_raw_varint32({}.get_cached_size())?;",
                    os, var
                ));
                w.write_line(&format!("{}.write_to_with_cached_sizes({})?;", var, os));
            }
            _ => {
                let param_type = self.os_write_fn_param_type();
                let os_write_fn_suffix = self.os_write_fn_suffix();
                let number = self.proto_field.number();
                w.write_line(&format!(
                    "{}.write_{}({}, {})?;",
                    os,
                    os_write_fn_suffix,
                    number,
                    ty.into_target(&param_type, var, &self.customize)
                ));
            }
        }
    }

    fn self_field(&self) -> String {
        format!("self.{}", self.rust_name)
    }

    fn self_field_is_some(&self) -> String {
        assert!(self.is_singular());
        format!("{}.is_some()", self.self_field())
    }

    fn self_field_is_not_empty(&self) -> String {
        assert!(self.is_repeated_or_map());
        format!("!{}.is_empty()", self.self_field())
    }

    fn self_field_is_none(&self) -> String {
        assert!(self.is_singular());
        format!("{}.is_none()", self.self_field())
    }

    // type of expression returned by `as_option()`
    fn as_option_type(&self) -> RustType {
        assert!(self.is_singular());
        match self.full_storage_type() {
            RustType::Option(ref e) if e.is_copy() => RustType::Option(e.clone()),
            RustType::Option(e) => RustType::Option(Box::new(e.ref_type())),
            RustType::SingularField(ty) | RustType::SingularPtrField(ty) => {
                RustType::Option(Box::new(RustType::Ref(ty)))
            }
            x => panic!("cannot convert {:?} to option", x),
        }
    }

    // field data viewed as Option
    fn self_field_as_option(&self) -> RustValueTyped {
        assert!(self.is_singular());

        let suffix = match self.full_storage_type() {
            RustType::Option(ref e) if e.is_copy() => "",
            _ => ".as_ref()",
        };

        self.as_option_type()
            .value(format!("{}{}", self.self_field(), suffix))
    }

    fn write_if_let_self_field_is_some<F>(&self, w: &mut CodeWriter, cb: F)
    where
        F: Fn(&str, &RustType, &mut CodeWriter),
    {
        match self.kind {
            FieldKind::Repeated(..) | FieldKind::Map(..) => panic!("field is not singular"),
            FieldKind::Singular(SingularField {
                flag: SingularFieldFlag::WithFlag { .. },
                ref elem,
            }) => {
                let var = "v";
                let ref_prefix = match elem.rust_storage_type().is_copy() {
                    true => "",
                    false => "ref ",
                };
                let as_option = self.self_field_as_option();
                w.if_let_stmt(
                    &format!("Some({}{})", ref_prefix, var),
                    &as_option.value,
                    |w| {
                        let v_type = as_option.rust_type.elem_type();
                        cb(var, &v_type, w);
                    },
                );
            }
            FieldKind::Singular(SingularField {
                flag: SingularFieldFlag::WithoutFlag,
                ref elem,
            }) => match *elem {
                FieldElem::Primitive(FieldDescriptorProto_Type::TYPE_STRING, ..)
                | FieldElem::Primitive(FieldDescriptorProto_Type::TYPE_BYTES, ..) => {
                    w.if_stmt(format!("!{}.is_empty()", self.self_field()), |w| {
                        cb(&self.self_field(), &self.full_storage_type(), w);
                    });
                }
                _ => {
                    w.if_stmt(
                        format!(
                            "{} != {}",
                            self.self_field(),
                            self.full_storage_type().default_value(&self.customize)
                        ),
                        |w| {
                            cb(&self.self_field(), &self.full_storage_type(), w);
                        },
                    );
                }
            },
            FieldKind::Oneof(..) => unreachable!(),
        }
    }

    fn write_if_self_field_is_not_empty<F>(&self, w: &mut CodeWriter, cb: F)
    where
        F: Fn(&mut CodeWriter),
    {
        assert!(self.is_repeated_or_map());
        let self_field_is_not_empty = self.self_field_is_not_empty();
        w.if_stmt(self_field_is_not_empty, cb);
    }

    pub fn write_if_self_field_is_none<F>(&self, w: &mut CodeWriter, cb: F)
    where
        F: Fn(&mut CodeWriter),
    {
        let self_field_is_none = self.self_field_is_none();
        w.if_stmt(self_field_is_none, cb)
    }

    // repeated or singular
    pub fn write_for_self_field<F>(&self, w: &mut CodeWriter, varn: &str, cb: F)
    where
        F: Fn(&mut CodeWriter, &RustType),
    {
        match self.kind {
            FieldKind::Oneof(OneofField {
                ref elem,
                ref oneof_type_name,
                ..
            }) => {
                let cond = format!(
                    "Some({}::{}(ref {}))",
                    oneof_type_name.to_code(&self.customize),
                    self.rust_name,
                    varn
                );
                w.if_let_stmt(&cond, &self.self_field_oneof(), |w| {
                    cb(w, &elem.rust_storage_type())
                })
            }
            _ => {
                let v_type = self.full_storage_iter_elem_type();
                let self_field = self.self_field();
                w.for_stmt(&format!("&{}", self_field), varn, |w| cb(w, &v_type));
            }
        }
    }

    fn write_self_field_assign(&self, w: &mut CodeWriter, value: &str) {
        let self_field = self.self_field();
        w.write_line(&format!("{} = {};", self_field, value));
    }

    fn write_self_field_assign_some(&self, w: &mut CodeWriter, value: &str) {
        let full_storage_type = self.full_storage_type();
        match self.singular() {
            &SingularField {
                flag: SingularFieldFlag::WithFlag { .. },
                ..
            } => {
                self.write_self_field_assign(
                    w,
                    &full_storage_type.wrap_value(value, &self.customize),
                );
            }
            &SingularField {
                flag: SingularFieldFlag::WithoutFlag,
                ..
            } => {
                self.write_self_field_assign(w, value);
            }
        }
    }

    fn write_self_field_assign_value(&self, w: &mut CodeWriter, value: &str, ty: &RustType) {
        match self.kind {
            FieldKind::Repeated(..) | FieldKind::Map(..) => {
                let converted = ty.into_target(&self.full_storage_type(), value, &self.customize);
                self.write_self_field_assign(w, &converted);
            }
            FieldKind::Singular(SingularField { ref elem, ref flag }) => {
                let converted = ty.into_target(&elem.rust_storage_type(), value, &self.customize);
                let wrapped = if *flag == SingularFieldFlag::WithoutFlag {
                    converted
                } else {
                    self.full_storage_type()
                        .wrap_value(&converted, &self.customize)
                };
                self.write_self_field_assign(w, &wrapped);
            }
            FieldKind::Oneof(..) => unreachable!(),
        }
    }

    fn write_self_field_assign_default(&self, w: &mut CodeWriter) {
        assert!(self.is_singular());
        if self.is_oneof() {
            let self_field_oneof = self.self_field_oneof();
            w.write_line(format!(
                "{} = ::std::option::Option::Some({}({}))",
                self_field_oneof,
                self.variant_path(),
                // TODO: default from .proto is not needed here (?)
                self.element_default_value_rust()
                    .into_type(self.full_storage_iter_elem_type(), &self.customize)
                    .value
            ));
        } else {
            // Note it is different from C++ protobuf, where field is initialized
            // with default value
            match self.full_storage_type() {
                RustType::SingularField(..) | RustType::SingularPtrField(..) => {
                    let self_field = self.self_field();
                    w.write_line(&format!("{}.set_default();", self_field));
                }
                _ => {
                    self.write_self_field_assign_some(
                        w,
                        &self
                            .elem()
                            .rust_storage_type()
                            .default_value_typed(&self.customize)
                            .into_type(self.elem().rust_storage_type(), &self.customize)
                            .value,
                    );
                }
            }
        }
    }

    fn self_field_vec_packed_fixed_data_size(&self) -> String {
        assert!(self.is_fixed());
        format!(
            "({}.len() * {}) as u32",
            self.self_field(),
            field_type_size(self.proto_type).unwrap()
        )
    }

    fn self_field_vec_packed_varint_data_size(&self) -> String {
        assert!(!self.is_fixed());
        let fn_name = if self.is_enum() {
            "vec_packed_enum_data_size".to_string()
        } else {
            let zigzag_suffix = if self.is_zigzag() { "_zigzag" } else { "" };
            format!("vec_packed_varint{}_data_size", zigzag_suffix)
        };
        format!(
            "{}::rt::{}(&{})",
            protobuf_crate_path(&self.customize),
            fn_name,
            self.self_field()
        )
    }

    fn self_field_vec_packed_data_size(&self) -> String {
        assert!(self.is_repeated_not_map());
        if self.is_fixed() {
            self.self_field_vec_packed_fixed_data_size()
        } else {
            self.self_field_vec_packed_varint_data_size()
        }
    }

    fn self_field_vec_packed_fixed_size(&self) -> String {
        // zero is filtered outside
        format!(
            "{} + {}::rt::compute_raw_varint32_size({}) + {}",
            self.tag_size(),
            protobuf_crate_path(&self.customize),
            self.self_field_vec_packed_fixed_data_size(),
            self.self_field_vec_packed_fixed_data_size()
        )
    }

    fn self_field_vec_packed_varint_size(&self) -> String {
        // zero is filtered outside
        assert!(!self.is_fixed());
        let fn_name = if self.is_enum() {
            "vec_packed_enum_size".to_string()
        } else {
            let zigzag_suffix = if self.is_zigzag() { "_zigzag" } else { "" };
            format!("vec_packed_varint{}_size", zigzag_suffix)
        };
        format!(
            "{}::rt::{}({}, &{})",
            protobuf_crate_path(&self.customize),
            fn_name,
            self.proto_field.number(),
            self.self_field()
        )
    }

    fn self_field_oneof(&self) -> String {
        format!("self.{}", self.oneof().oneof_rust_field_name)
    }

    pub fn clear_field_func(&self) -> String {
        format!("clear_{}", self.rust_name)
    }

    // Write `merge_from` part for this singular or repeated field
    // of type message, string or bytes
    fn write_merge_from_field_message_string_bytes(&self, w: &mut CodeWriter) {
        let singular_or_repeated = match self.kind {
            FieldKind::Repeated(..) => "repeated",
            FieldKind::Singular(SingularField {
                flag: SingularFieldFlag::WithFlag { .. },
                ..
            }) => "singular",
            FieldKind::Singular(SingularField {
                flag: SingularFieldFlag::WithoutFlag,
                ..
            }) => "singular_proto3",
            FieldKind::Map(..) | FieldKind::Oneof(..) => unreachable!(),
        };
        let carllerche = match self.kind.primitive_type_variant() {
            PrimitiveTypeVariant::Carllerche => "carllerche_",
            PrimitiveTypeVariant::Default => "",
        };
        let type_name_for_fn = protobuf_name(self.proto_type);
        w.write_line(&format!(
            "{}::rt::read_{}_{}{}_into(wire_type, is, &mut self.{})?;",
            protobuf_crate_path(&self.customize),
            singular_or_repeated,
            carllerche,
            type_name_for_fn,
            self.rust_name
        ));
    }

    fn write_error_unexpected_wire_type(&self, wire_type_var: &str, w: &mut CodeWriter) {
        w.write_line(&format!(
            "return ::std::result::Result::Err({}::rt::unexpected_wire_type({}));",
            protobuf_crate_path(&self.customize),
            wire_type_var
        ));
    }

    fn write_assert_wire_type(&self, wire_type_var: &str, w: &mut CodeWriter) {
        w.if_stmt(
            &format!(
                "{} != {}::wire_format::{:?}",
                wire_type_var,
                protobuf_crate_path(&self.customize),
                self.wire_type
            ),
            |w| {
                self.write_error_unexpected_wire_type(wire_type_var, w);
            },
        );
    }

    // Write `merge_from` part for this oneof field
    fn write_merge_from_oneof(&self, f: &OneofField, wire_type_var: &str, w: &mut CodeWriter) {
        self.write_assert_wire_type(wire_type_var, w);

        let typed = RustValueTyped {
            value: format!(
                "{}?",
                self.proto_type.read("is", f.elem.primitive_type_variant())
            ),
            rust_type: self.full_storage_iter_elem_type(),
        };

        let maybe_boxed = if f.boxed {
            typed.boxed(&self.customize)
        } else {
            typed
        };

        w.write_line(&format!(
            "self.{} = ::std::option::Option::Some({}({}));",
            self.oneof().oneof_rust_field_name,
            self.variant_path(),
            maybe_boxed.value
        )); // TODO: into_type
    }

    // Write `merge_from` part for this map field
    fn write_merge_from_map(&self, w: &mut CodeWriter) {
        let &MapField {
            ref key, ref value, ..
        } = self.map();
        w.write_line(&format!(
            "{}::rt::read_map_into::<{}, {}>(wire_type, is, &mut {})?;",
            protobuf_crate_path(&self.customize),
            key.lib_protobuf_type(&self.customize),
            value.lib_protobuf_type(&self.customize),
            self.self_field()
        ));
    }

    // Write `merge_from` part for this singular field
    fn write_merge_from_singular(&self, wire_type_var: &str, w: &mut CodeWriter) {
        let field = match self.kind {
            FieldKind::Singular(ref field) => field,
            _ => panic!(),
        };

        match field.elem {
            FieldElem::Message(..)
            | FieldElem::Primitive(FieldDescriptorProto_Type::TYPE_STRING, ..)
            | FieldElem::Primitive(FieldDescriptorProto_Type::TYPE_BYTES, ..) => {
                self.write_merge_from_field_message_string_bytes(w);
            }
            FieldElem::Enum(..) => {
                let version = match field.flag {
                    SingularFieldFlag::WithFlag { .. } => "proto2",
                    SingularFieldFlag::WithoutFlag => "proto3",
                };
                w.write_line(&format!(
                    "{}::rt::read_{}_enum_with_unknown_fields_into({}, is, &mut self.{}, {}, &mut self.unknown_fields)?",
                    protobuf_crate_path(&self.customize),
                    version,
                    wire_type_var,
                    self.rust_name,
                    self.proto_field.number()
                ));
            }
            _ => {
                let read_proc = format!(
                    "{}?",
                    self.proto_type.read("is", PrimitiveTypeVariant::Default)
                );

                self.write_assert_wire_type(wire_type_var, w);
                w.write_line(&format!("let tmp = {};", read_proc));
                self.write_self_field_assign_some(w, "tmp");
            }
        }
    }

    // Write `merge_from` part for this repeated field
    fn write_merge_from_repeated(&self, wire_type_var: &str, w: &mut CodeWriter) {
        let field = match self.kind {
            FieldKind::Repeated(ref field) => field,
            _ => panic!(),
        };

        match field.elem {
            FieldElem::Message(..)
            | FieldElem::Primitive(FieldDescriptorProto_Type::TYPE_STRING, ..)
            | FieldElem::Primitive(FieldDescriptorProto_Type::TYPE_BYTES, ..) => {
                self.write_merge_from_field_message_string_bytes(w);
            }
            FieldElem::Enum(..) => {
                w.write_line(&format!(
                    "{}::rt::read_repeated_enum_with_unknown_fields_into({}, is, &mut self.{}, {}, &mut self.unknown_fields)?",
                    protobuf_crate_path(&self.customize),
                    wire_type_var,
                    self.rust_name,
                    self.proto_field.number()
                ));
            }
            _ => {
                w.write_line(&format!(
                    "{}::rt::read_repeated_{}_into({}, is, &mut self.{})?;",
                    protobuf_crate_path(&self.customize),
                    protobuf_name(self.proto_type),
                    wire_type_var,
                    self.rust_name
                ));
            }
        }
    }

    // Write `merge_from` part for this field
    pub fn write_merge_from_field(&self, wire_type_var: &str, w: &mut CodeWriter) {
        match self.kind {
            FieldKind::Oneof(ref f) => self.write_merge_from_oneof(&f, wire_type_var, w),
            FieldKind::Map(..) => self.write_merge_from_map(w),
            FieldKind::Singular(..) => self.write_merge_from_singular(wire_type_var, w),
            FieldKind::Repeated(..) => self.write_merge_from_repeated(wire_type_var, w),
        }
    }

    fn self_field_vec_packed_size(&self) -> String {
        match self.kind {
            FieldKind::Repeated(RepeatedField { packed: true, .. }) => {
                // zero is filtered outside
                if self.is_fixed() {
                    self.self_field_vec_packed_fixed_size()
                } else {
                    self.self_field_vec_packed_varint_size()
                }
            }
            _ => {
                panic!("not packed");
            }
        }
    }

    pub fn write_element_size(
        &self,
        w: &mut CodeWriter,
        item_var: &str,
        item_var_type: &RustType,
        sum_var: &str,
    ) {
        assert!(!self.is_repeated_packed());

        match self.proto_type {
            FieldDescriptorProto_Type::TYPE_MESSAGE => {
                w.write_line(&format!("let len = {}.compute_size();", item_var));
                let tag_size = self.tag_size();
                w.write_line(&format!(
                    "{} += {} + {}::rt::compute_raw_varint32_size(len) + len;",
                    sum_var,
                    tag_size,
                    protobuf_crate_path(&self.customize)
                ));
            }
            _ => {
                w.write_line(&format!(
                    "{} += {};",
                    sum_var,
                    self.element_size(item_var, item_var_type)
                ));
            }
        }
    }

    pub fn write_message_write_field(&self, w: &mut CodeWriter) {
        match self.kind {
            FieldKind::Singular(..) => {
                self.write_if_let_self_field_is_some(w, |v, v_type, w| {
                    self.write_write_element(w, "os", v, v_type);
                });
            }
            FieldKind::Repeated(RepeatedField { packed: false, .. }) => {
                self.write_for_self_field(w, "v", |w, v_type| {
                    self.write_write_element(w, "os", "v", v_type);
                });
            }
            FieldKind::Repeated(RepeatedField { packed: true, .. }) => {
                self.write_if_self_field_is_not_empty(w, |w| {
                    let number = self.proto_field.number();
                    w.write_line(&format!(
                        "os.write_tag({}, {}::wire_format::{:?})?;",
                        number,
                        protobuf_crate_path(&self.customize),
                        wire_format::WireTypeLengthDelimited
                    ));
                    w.comment("TODO: Data size is computed again, it should be cached");
                    let data_size_expr = self.self_field_vec_packed_data_size();
                    w.write_line(&format!("os.write_raw_varint32({})?;", data_size_expr));
                    self.write_for_self_field(w, "v", |w, v_type| {
                        let param_type = self.os_write_fn_param_type();
                        let os_write_fn_suffix = self.os_write_fn_suffix();
                        w.write_line(&format!(
                            "os.write_{}_no_tag({})?;",
                            os_write_fn_suffix,
                            v_type.into_target(&param_type, "v", &self.customize)
                        ));
                    });
                });
            }
            FieldKind::Map(MapField {
                ref key, ref value, ..
            }) => {
                w.write_line(&format!(
                    "{}::rt::write_map_with_cached_sizes::<{}, {}>({}, &{}, os)?;",
                    protobuf_crate_path(&self.customize),
                    key.lib_protobuf_type(&self.customize),
                    value.lib_protobuf_type(&self.customize),
                    self.proto_field.number(),
                    self.self_field()
                ));
            }
            FieldKind::Oneof(..) => unreachable!(),
        };
    }

    pub fn write_message_compute_field_size(&self, sum_var: &str, w: &mut CodeWriter) {
        match self.kind {
            FieldKind::Singular(..) => {
                self.write_if_let_self_field_is_some(w, |v, v_type, w| {
                    match field_type_size(self.proto_type) {
                        Some(s) => {
                            let tag_size = self.tag_size();
                            w.write_line(&format!("{} += {};", sum_var, (s + tag_size) as isize));
                        }
                        None => {
                            self.write_element_size(w, v, v_type, sum_var);
                        }
                    };
                });
            }
            FieldKind::Repeated(RepeatedField { packed: false, .. }) => {
                match field_type_size(self.proto_type) {
                    Some(s) => {
                        let tag_size = self.tag_size();
                        let self_field = self.self_field();
                        w.write_line(&format!(
                            "{} += {} * {}.len() as u32;",
                            sum_var,
                            (s + tag_size) as isize,
                            self_field
                        ));
                    }
                    None => {
                        self.write_for_self_field(w, "value", |w, value_type| {
                            self.write_element_size(w, "value", value_type, sum_var);
                        });
                    }
                };
            }
            FieldKind::Map(MapField {
                ref key, ref value, ..
            }) => {
                w.write_line(&format!(
                    "{} += {}::rt::compute_map_size::<{}, {}>({}, &{});",
                    sum_var,
                    protobuf_crate_path(&self.customize),
                    key.lib_protobuf_type(&self.customize),
                    value.lib_protobuf_type(&self.customize),
                    self.proto_field.number(),
                    self.self_field()
                ));
            }
            FieldKind::Repeated(RepeatedField { packed: true, .. }) => {
                self.write_if_self_field_is_not_empty(w, |w| {
                    let size_expr = self.self_field_vec_packed_size();
                    w.write_line(&format!("{} += {};", sum_var, size_expr));
                });
            }
            FieldKind::Oneof(..) => unreachable!(),
        }
    }

    fn write_message_field_get_singular(&self, w: &mut CodeWriter) {
        let get_xxx_return_type = self.get_xxx_return_type();

        if self.proto_type == FieldDescriptorProto_Type::TYPE_MESSAGE {
            let self_field = self.self_field();
            let ref rust_type_message = match self.elem().rust_storage_type() {
                RustType::Message(m) => m,
                _ => unreachable!(),
            };
            w.write_line(&format!(
                "{}.as_ref().unwrap_or_else(|| {})",
                self_field,
                rust_type_message.default_instance(&self.customize)
            ));
        } else {
            let get_xxx_default_value_rust = self.get_xxx_default_value_rust();
            let self_field = self.self_field();
            match self.singular() {
                &SingularField {
                    flag: SingularFieldFlag::WithFlag { .. },
                    ..
                } => {
                    if get_xxx_return_type.is_ref() {
                        let as_option = self.self_field_as_option();
                        w.match_expr(&as_option.value, |w| {
                            let v_type = as_option.rust_type.elem_type();
                            let r_type = self.get_xxx_return_type();
                            w.case_expr(
                                "Some(v)",
                                v_type.into_target(&r_type, "v", &self.customize),
                            );
                            let get_xxx_default_value_rust = self.get_xxx_default_value_rust();
                            w.case_expr("None", get_xxx_default_value_rust);
                        });
                    } else {
                        w.write_line(&format!(
                            "{}.unwrap_or({})",
                            self_field, get_xxx_default_value_rust
                        ));
                    }
                }
                &SingularField {
                    flag: SingularFieldFlag::WithoutFlag,
                    ..
                } => {
                    w.write_line(self.full_storage_type().into_target(
                        &get_xxx_return_type,
                        &self_field,
                        &self.customize,
                    ));
                }
            }
        }
    }

    fn write_message_field_get(&self, w: &mut CodeWriter) {
        let get_xxx_return_type = self.get_xxx_return_type();
        let fn_def = format!(
            "get_{}(&self) -> {}",
            self.rust_name,
            get_xxx_return_type.to_code(&self.customize)
        );

        w.pub_fn(&fn_def, |w| match self.kind {
            FieldKind::Oneof(OneofField { ref elem, .. }) => {
                let self_field_oneof = self.self_field_oneof();
                w.match_expr(self_field_oneof, |w| {
                    let (refv, vtype) = if !self.elem_type_is_copy() {
                        ("ref v", elem.rust_storage_type().ref_type())
                    } else {
                        ("v", elem.rust_storage_type())
                    };
                    w.case_expr(
                        format!(
                            "::std::option::Option::Some({}({}))",
                            self.variant_path(),
                            refv
                        ),
                        vtype.into_target(&get_xxx_return_type, "v", &self.customize),
                    );
                    w.case_expr("_", self.get_xxx_default_value_rust());
                })
            }
            FieldKind::Singular(..) => {
                self.write_message_field_get_singular(w);
            }
            FieldKind::Repeated(..) | FieldKind::Map(..) => {
                let self_field = self.self_field();
                w.write_line(&format!("&{}", self_field));
            }
        });
    }

    fn has_has(&self) -> bool {
        match self.kind {
            FieldKind::Repeated(..) | FieldKind::Map(..) => false,
            FieldKind::Singular(SingularField {
                flag: SingularFieldFlag::WithFlag { .. },
                ..
            }) => true,
            FieldKind::Singular(SingularField {
                flag: SingularFieldFlag::WithoutFlag,
                ..
            }) => false,
            FieldKind::Oneof(..) => true,
        }
    }

    fn has_mut(&self) -> bool {
        match self.kind {
            FieldKind::Repeated(..) | FieldKind::Map(..) => true,
            // TODO: string should be public, and mut is not needed
            FieldKind::Singular(..) | FieldKind::Oneof(..) => !self.elem_type_is_copy(),
        }
    }

    fn has_take(&self) -> bool {
        match self.kind {
            FieldKind::Repeated(..) | FieldKind::Map(..) => true,
            // TODO: string should be public, and mut is not needed
            FieldKind::Singular(..) | FieldKind::Oneof(..) => !self.elem_type_is_copy(),
        }
    }

    fn has_name(&self) -> String {
        format!("has_{}", self.rust_name)
    }

    fn write_message_field_has(&self, w: &mut CodeWriter) {
        w.pub_fn(&format!("{}(&self) -> bool", self.has_name()), |w| {
            if !self.is_oneof() {
                let self_field_is_some = self.self_field_is_some();
                w.write_line(self_field_is_some);
            } else {
                let self_field_oneof = self.self_field_oneof();
                w.match_expr(self_field_oneof, |w| {
                    w.case_expr(
                        format!("::std::option::Option::Some({}(..))", self.variant_path()),
                        "true",
                    );
                    w.case_expr("_", "false");
                });
            }
        });
    }

    fn write_message_field_set(&self, w: &mut CodeWriter) {
        let set_xxx_param_type = self.set_xxx_param_type();
        w.comment("Param is passed by value, moved");
        let ref name = self.rust_name;
        w.pub_fn(
            &format!(
                "set_{}(&mut self, v: {})",
                name,
                set_xxx_param_type.to_code(&self.customize)
            ),
            |w| {
                if !self.is_oneof() {
                    self.write_self_field_assign_value(w, "v", &set_xxx_param_type);
                } else {
                    let self_field_oneof = self.self_field_oneof();
                    let v = set_xxx_param_type.into_target(
                        &self.oneof().rust_type(),
                        "v",
                        &self.customize,
                    );
                    w.write_line(&format!(
                        "{} = ::std::option::Option::Some({}({}))",
                        self_field_oneof,
                        self.variant_path(),
                        v
                    ));
                }
            },
        );
    }

    fn write_message_field_mut(&self, w: &mut CodeWriter) {
        let mut_xxx_return_type = self.mut_xxx_return_type();
        w.comment("Mutable pointer to the field.");
        if self.is_singular() {
            w.comment("If field is not initialized, it is initialized with default value first.");
        }
        let fn_def = match mut_xxx_return_type {
            RustType::Ref(ref param) => format!(
                "mut_{}(&mut self) -> &mut {}",
                self.rust_name,
                param.to_code(&self.customize)
            ),
            _ => panic!(
                "not a ref: {}",
                mut_xxx_return_type.to_code(&self.customize)
            ),
        };
        w.pub_fn(&fn_def, |w| {
            match self.kind {
                FieldKind::Repeated(..) | FieldKind::Map(..) => {
                    let self_field = self.self_field();
                    w.write_line(&format!("&mut {}", self_field));
                }
                FieldKind::Singular(SingularField {
                    flag: SingularFieldFlag::WithFlag { .. },
                    ..
                }) => {
                    self.write_if_self_field_is_none(w, |w| {
                        self.write_self_field_assign_default(w);
                    });
                    let self_field = self.self_field();
                    w.write_line(&format!("{}.as_mut().unwrap()", self_field));
                }
                FieldKind::Singular(SingularField {
                    flag: SingularFieldFlag::WithoutFlag,
                    ..
                }) => w.write_line(&format!("&mut {}", self.self_field())),
                FieldKind::Oneof(..) => {
                    let self_field_oneof = self.self_field_oneof();

                    // if oneof does not contain current field
                    w.if_let_else_stmt(
                        &format!("::std::option::Option::Some({}(_))", self.variant_path())[..],
                        &self_field_oneof[..],
                        |w| {
                            // initialize it with default value
                            w.write_line(&format!(
                                "{} = ::std::option::Option::Some({}({}));",
                                self_field_oneof,
                                self.variant_path(),
                                self.element_default_value_rust()
                                    .into_type(self.oneof().rust_type(), &self.customize)
                                    .value
                            ));
                        },
                    );

                    // extract field
                    w.match_expr(self_field_oneof, |w| {
                        w.case_expr(
                            format!(
                                "::std::option::Option::Some({}(ref mut v))",
                                self.variant_path()
                            ),
                            "v",
                        );
                        w.case_expr("_", "panic!()");
                    });
                }
            }
        });
    }

    fn write_message_field_take_oneof(&self, w: &mut CodeWriter) {
        let take_xxx_return_type = self.take_xxx_return_type();

        // TODO: replace with if let
        w.write_line(&format!("if self.{}() {{", self.has_name()));
        w.indented(|w| {
            let self_field_oneof = self.self_field_oneof();
            w.match_expr(format!("{}.take()", self_field_oneof), |w| {
                let value_in_some = self.oneof().rust_type().value("v".to_owned());
                let converted =
                    value_in_some.into_type(self.take_xxx_return_type(), &self.customize);
                w.case_expr(
                    format!("::std::option::Option::Some({}(v))", self.variant_path()),
                    &converted.value,
                );
                w.case_expr("_", "panic!()");
            });
        });
        w.write_line("} else {");
        w.indented(|w| {
            w.write_line(
                self.elem()
                    .rust_storage_type()
                    .default_value_typed(&self.customize)
                    .into_type(take_xxx_return_type.clone(), &self.customize)
                    .value,
            );
        });
        w.write_line("}");
    }

    fn write_message_field_take(&self, w: &mut CodeWriter) {
        let take_xxx_return_type = self.take_xxx_return_type();
        w.comment("Take field");
        w.pub_fn(
            &format!(
                "take_{}(&mut self) -> {}",
                self.rust_name,
                take_xxx_return_type.to_code(&self.customize)
            ),
            |w| match self.kind {
                FieldKind::Oneof(..) => {
                    self.write_message_field_take_oneof(w);
                }
                FieldKind::Repeated(..) | FieldKind::Map(..) => {
                    w.write_line(&format!(
                        "::std::mem::replace(&mut self.{}, {})",
                        self.rust_name,
                        take_xxx_return_type.default_value(&self.customize)
                    ));
                }
                FieldKind::Singular(SingularField {
                    ref elem,
                    flag: SingularFieldFlag::WithFlag { .. },
                }) => {
                    if !elem.is_copy() {
                        w.write_line(&format!(
                            "{}.take().unwrap_or_else(|| {})",
                            self.self_field(),
                            elem.rust_storage_type().default_value(&self.customize)
                        ));
                    } else {
                        w.write_line(&format!(
                            "{}.take().unwrap_or({})",
                            self.self_field(),
                            self.element_default_value_rust().value
                        ));
                    }
                }
                FieldKind::Singular(SingularField {
                    flag: SingularFieldFlag::WithoutFlag,
                    ..
                }) => w.write_line(&format!(
                    "::std::mem::replace(&mut {}, {})",
                    self.self_field(),
                    self.full_storage_type().default_value(&self.customize)
                )),
            },
        );
    }

    pub fn write_message_single_field_accessors(&self, w: &mut CodeWriter) {
        // TODO: do not generate `get` when !proto2 and !generate_accessors`
        w.write_line("");
        self.write_message_field_get(w);

        if !self.generate_accessors {
            return;
        }

        let clear_field_func = self.clear_field_func();
        w.pub_fn(&format!("{}(&mut self)", clear_field_func), |w| {
            self.write_clear(w);
        });

        if self.has_has() {
            w.write_line("");
            self.write_message_field_has(w);
        }

        w.write_line("");
        self.write_message_field_set(w);

        if self.has_mut() {
            w.write_line("");
            self.write_message_field_mut(w);
        }

        if self.has_take() {
            w.write_line("");
            self.write_message_field_take(w);
        }
    }
}

pub(crate) fn rust_field_name_for_protobuf_field_name(name: &str) -> RustIdent {
    if rust::is_rust_keyword(name) {
        RustIdent::new(&format!("field_{}", name))
    } else {
        RustIdent::new(name)
    }
}
