// Should not be a part of public API
#![doc(hidden)]

use crate::descriptor::DescriptorProto;
use crate::descriptor::EnumDescriptorProto;
use crate::descriptor::EnumValueDescriptorProto;
use crate::descriptor::FieldDescriptorProto;
/// utilities to work with descriptor
use crate::descriptor::FileDescriptorProto;
use crate::descriptor::OneofDescriptorProto;
use crate::rust;
use crate::strx;

// Copy-pasted from libsyntax.
fn ident_start(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
}

// Copy-pasted from libsyntax.
fn ident_continue(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_'
}

pub fn proto_path_to_rust_mod(path: &str) -> String {
    let without_dir = strx::remove_to(path, '/');
    let without_suffix = strx::remove_suffix(without_dir, ".proto");

    let name = without_suffix
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let valid = if i == 0 {
                ident_start(c)
            } else {
                ident_continue(c)
            };
            if valid {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    let name = if rust::is_rust_keyword(&name) {
        format!("{}_pb", name)
    } else {
        name
    };
    name
}

pub struct RootScope<'a> {
    pub file_descriptors: &'a [FileDescriptorProto],
}

impl<'a> RootScope<'a> {
    fn packages(&'a self) -> Vec<FileScope<'a>> {
        self.file_descriptors
            .iter()
            .map(|fd| FileScope {
                file_descriptor: fd,
            })
            .collect()
    }

    // find enum by fully qualified name
    pub fn find_enum(&'a self, fqn: &str) -> EnumWithScope<'a> {
        match self.find_message_or_enum(fqn) {
            MessageOrEnumWithScope::Enum(e) => e,
            _ => panic!("not an enum: {}", fqn),
        }
    }

    // find message by fully qualified name
    pub fn find_message(&'a self, fqn: &str) -> MessageWithScope<'a> {
        match self.find_message_or_enum(fqn) {
            MessageOrEnumWithScope::Message(m) => m,
            _ => panic!("not a message: {}", fqn),
        }
    }

    // find message or enum by fully qualified name
    pub fn find_message_or_enum(&'a self, fqn: &str) -> MessageOrEnumWithScope<'a> {
        assert!(fqn.starts_with("."), "name must start with dot: {}", fqn);
        let fqn1 = &fqn[1..];
        self.packages()
            .into_iter()
            .flat_map(|p| {
                (if p.get_package().is_empty() {
                    p.find_message_or_enum(fqn1)
                } else if fqn1.starts_with(&(p.get_package().to_string() + ".")) {
                    let remaining = &fqn1[(p.get_package().len() + 1)..];
                    p.find_message_or_enum(remaining)
                } else {
                    None
                })
                .into_iter()
            })
            .next()
            .expect(&format!("enum not found by name: {}", fqn))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Syntax {
    PROTO2,
    PROTO3,
}

impl Syntax {
    pub fn parse(s: &str) -> Self {
        match s {
            "" | "proto2" => Syntax::PROTO2,
            "proto3" => Syntax::PROTO3,
            _ => panic!("unsupported syntax value: {:?}", s),
        }
    }
}

#[derive(Clone)]
pub struct FileScope<'a> {
    pub file_descriptor: &'a FileDescriptorProto,
}

impl<'a> FileScope<'a> {
    fn get_package(&self) -> &'a str {
        self.file_descriptor.get_package()
    }

    pub fn syntax(&self) -> Syntax {
        Syntax::parse(self.file_descriptor.get_syntax())
    }

    pub fn to_scope(&self) -> Scope<'a> {
        Scope {
            file_scope: self.clone(),
            path: Vec::new(),
        }
    }

    fn find_message_or_enum(&self, name: &str) -> Option<MessageOrEnumWithScope<'a>> {
        assert!(!name.starts_with("."));
        self.find_messages_and_enums()
            .into_iter()
            .filter(|e| e.name_to_package() == name)
            .next()
    }

    // find all enums in given file descriptor
    pub fn find_enums(&self) -> Vec<EnumWithScope<'a>> {
        let mut r = Vec::new();

        self.to_scope().walk_scopes(|scope| {
            r.extend(scope.get_enums());
        });

        r
    }

    // find all messages in given file descriptor
    pub fn find_messages(&self) -> Vec<MessageWithScope<'a>> {
        let mut r = Vec::new();

        self.to_scope().walk_scopes(|scope| {
            r.extend(scope.get_messages());
        });

        r
    }

    // find all messages and enums in given file descriptor
    pub fn find_messages_and_enums(&self) -> Vec<MessageOrEnumWithScope<'a>> {
        let mut r = Vec::new();

        self.to_scope().walk_scopes(|scope| {
            r.extend(scope.get_messages_and_enums());
        });

        r
    }
}

#[derive(Clone)]
pub struct Scope<'a> {
    pub file_scope: FileScope<'a>,
    pub path: Vec<&'a DescriptorProto>,
}

impl<'a> Scope<'a> {
    pub fn get_file_descriptor(&self) -> &'a FileDescriptorProto {
        self.file_scope.file_descriptor
    }

    // get message descriptors in this scope
    fn get_message_descriptors(&self) -> &'a [DescriptorProto] {
        if self.path.is_empty() {
            self.file_scope.file_descriptor.get_message_type()
        } else {
            self.path.last().unwrap().get_nested_type()
        }
    }

    // get enum descriptors in this scope
    fn get_enum_descriptors(&self) -> &'a [EnumDescriptorProto] {
        if self.path.is_empty() {
            self.file_scope.file_descriptor.get_enum_type()
        } else {
            self.path.last().unwrap().get_enum_type()
        }
    }

    // get messages with attached scopes in this scope
    pub fn get_messages(&self) -> Vec<MessageWithScope<'a>> {
        self.get_message_descriptors()
            .iter()
            .map(|m| MessageWithScope {
                scope: self.clone(),
                message: m,
            })
            .collect()
    }

    // get enums with attached scopes in this scope
    pub fn get_enums(&self) -> Vec<EnumWithScope<'a>> {
        self.get_enum_descriptors()
            .iter()
            .map(|e| EnumWithScope {
                scope: self.clone(),
                en: e,
            })
            .collect()
    }

    // get messages and enums with attached scopes in this scope
    pub fn get_messages_and_enums(&self) -> Vec<MessageOrEnumWithScope<'a>> {
        self.get_messages()
            .into_iter()
            .map(|m| MessageOrEnumWithScope::Message(m))
            .chain(
                self.get_enums()
                    .into_iter()
                    .map(|m| MessageOrEnumWithScope::Enum(m)),
            )
            .collect()
    }

    // nested scopes, i. e. scopes of nested messages
    fn nested_scopes(&self) -> Vec<Scope<'a>> {
        self.get_message_descriptors()
            .iter()
            .map(|m| {
                let mut nested = self.clone();
                nested.path.push(m);
                nested
            })
            .collect()
    }

    fn walk_scopes_impl<F: FnMut(&Scope<'a>)>(&self, callback: &mut F) {
        (*callback)(self);

        for nested in self.nested_scopes() {
            nested.walk_scopes_impl(callback);
        }
    }

    // apply callback for this scope and all nested scopes
    fn walk_scopes<F>(&self, mut callback: F)
    where
        F: FnMut(&Scope<'a>),
    {
        self.walk_scopes_impl(&mut callback);
    }

    pub fn prefix(&self) -> String {
        if self.path.is_empty() {
            "".to_string()
        } else {
            let v: Vec<&'a str> = self.path.iter().map(|m| m.get_name()).collect();
            let mut r = v.join(".");
            r.push_str(".");
            r
        }
    }

    // rust type name prefix for this scope
    pub fn rust_prefix(&self) -> String {
        self.prefix().replace(".", "_")
    }
}

pub trait WithScope<'a> {
    fn get_scope(&self) -> &Scope<'a>;

    fn get_file_descriptor(&self) -> &'a FileDescriptorProto {
        self.get_scope().get_file_descriptor()
    }

    // message or enum name
    fn get_name(&self) -> &'a str;

    fn escape_prefix(&self) -> &'static str;

    fn name_to_package(&self) -> String {
        let mut r = self.get_scope().prefix();
        r.push_str(self.get_name());
        r
    }

    /// Return absolute name starting with dot
    fn name_absolute(&self) -> String {
        let mut r = String::new();
        r.push_str(".");
        let package = self.get_file_descriptor().get_package();
        if !package.is_empty() {
            r.push_str(package);
            r.push_str(".");
        }
        r.push_str(&self.name_to_package());
        r
    }

    // rust type name of this descriptor
    fn rust_name(&self) -> String {
        let mut r = self.get_scope().rust_prefix();
        // Only escape if prefix is not empty
        if r.is_empty() && rust::is_rust_keyword(self.get_name()) {
            r.push_str(self.escape_prefix());
        }
        r.push_str(self.get_name());
        r
    }

    // fully-qualified name of this type
    fn rust_fq_name(&self) -> String {
        format!(
            "{}::{}",
            proto_path_to_rust_mod(self.get_scope().get_file_descriptor().get_name()),
            self.rust_name()
        )
    }
}

#[derive(Clone)]
pub struct MessageWithScope<'a> {
    pub scope: Scope<'a>,
    pub message: &'a DescriptorProto,
}

impl<'a> WithScope<'a> for MessageWithScope<'a> {
    fn get_scope(&self) -> &Scope<'a> {
        &self.scope
    }

    fn escape_prefix(&self) -> &'static str {
        "message_"
    }

    fn get_name(&self) -> &'a str {
        self.message.get_name()
    }
}

impl<'a> MessageWithScope<'a> {
    pub fn into_scope(mut self) -> Scope<'a> {
        self.scope.path.push(self.message);
        self.scope
    }

    pub fn to_scope(&self) -> Scope<'a> {
        self.clone().into_scope()
    }

    pub fn fields(&self) -> Vec<FieldWithContext<'a>> {
        self.message
            .get_field()
            .iter()
            .map(|f| FieldWithContext {
                field: f,
                message: self.clone(),
            })
            .collect()
    }

    pub fn oneofs(&self) -> Vec<OneofWithContext<'a>> {
        self.message
            .get_oneof_decl()
            .iter()
            .enumerate()
            .map(|(index, oneof)| OneofWithContext {
                message: self.clone(),
                oneof: &oneof,
                index: index as u32,
            })
            .collect()
    }

    pub fn oneof_by_index(&self, index: u32) -> OneofWithContext<'a> {
        self.oneofs().swap_remove(index as usize)
    }

    /// Pair of (key, value) if this message is map entry
    pub fn map_entry(&'a self) -> Option<(FieldWithContext<'a>, FieldWithContext<'a>)> {
        if self.message.get_options().get_map_entry() {
            let key = self
                .fields()
                .into_iter()
                .find(|f| f.field.get_number() == 1)
                .unwrap();
            let value = self
                .fields()
                .into_iter()
                .find(|f| f.field.get_number() == 2)
                .unwrap();
            Some((key, value))
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct EnumWithScope<'a> {
    pub scope: Scope<'a>,
    pub en: &'a EnumDescriptorProto,
}

impl<'a> EnumWithScope<'a> {
    // enum values
    pub fn values(&'a self) -> &'a [EnumValueDescriptorProto] {
        self.en.get_value()
    }

    // find enum value by name
    pub fn value_by_name(&'a self, name: &str) -> &'a EnumValueDescriptorProto {
        self.en
            .get_value()
            .into_iter()
            .find(|v| v.get_name() == name)
            .unwrap()
    }
}

pub trait EnumValueDescriptorEx {
    fn rust_name(&self) -> String;
}

impl EnumValueDescriptorEx for EnumValueDescriptorProto {
    fn rust_name(&self) -> String {
        let mut r = String::new();
        if rust::is_rust_keyword(self.get_name()) {
            r.push_str("value_");
        }
        r.push_str(self.get_name());
        r
    }
}

impl<'a> WithScope<'a> for EnumWithScope<'a> {
    fn get_scope(&self) -> &Scope<'a> {
        &self.scope
    }

    fn escape_prefix(&self) -> &'static str {
        "enum_"
    }

    fn get_name(&self) -> &'a str {
        self.en.get_name()
    }
}

pub enum MessageOrEnumWithScope<'a> {
    Message(MessageWithScope<'a>),
    Enum(EnumWithScope<'a>),
}

impl<'a> WithScope<'a> for MessageOrEnumWithScope<'a> {
    fn get_scope(&self) -> &Scope<'a> {
        match self {
            &MessageOrEnumWithScope::Message(ref m) => m.get_scope(),
            &MessageOrEnumWithScope::Enum(ref e) => e.get_scope(),
        }
    }

    fn escape_prefix(&self) -> &'static str {
        match self {
            &MessageOrEnumWithScope::Message(ref m) => m.escape_prefix(),
            &MessageOrEnumWithScope::Enum(ref e) => e.escape_prefix(),
        }
    }

    fn get_name(&self) -> &'a str {
        match self {
            &MessageOrEnumWithScope::Message(ref m) => m.get_name(),
            &MessageOrEnumWithScope::Enum(ref e) => e.get_name(),
        }
    }
}

pub trait FieldDescriptorProtoExt {
    fn rust_name(&self) -> String;
}

impl FieldDescriptorProtoExt for FieldDescriptorProto {
    fn rust_name(&self) -> String {
        if rust::is_rust_keyword(self.get_name()) {
            format!("field_{}", self.get_name())
        } else {
            self.get_name().to_string()
        }
    }
}

#[derive(Clone)]
pub struct FieldWithContext<'a> {
    pub field: &'a FieldDescriptorProto,
    pub message: MessageWithScope<'a>,
}

impl<'a> FieldWithContext<'a> {
    #[doc(hidden)]
    pub fn is_oneof(&self) -> bool {
        self.field.has_oneof_index()
    }

    pub fn oneof(&self) -> Option<OneofWithContext<'a>> {
        if self.is_oneof() {
            Some(
                self.message
                    .oneof_by_index(self.field.get_oneof_index() as u32),
            )
        } else {
            None
        }
    }

    pub fn number(&self) -> u32 {
        self.field.get_number() as u32
    }

    /// Shortcut
    pub fn name(&self) -> &str {
        self.field.get_name()
    }

    // field name in generated code
    #[deprecated]
    pub fn rust_name(&self) -> String {
        self.field.rust_name()
    }

    // From field to file root
    pub fn containing_messages(&self) -> Vec<&'a DescriptorProto> {
        let mut r = Vec::new();
        r.push(self.message.message);
        r.extend(self.message.scope.path.iter().rev());
        r
    }
}

#[derive(Clone)]
pub struct OneofVariantWithContext<'a> {
    pub oneof: &'a OneofWithContext<'a>,
    pub field: &'a FieldDescriptorProto,
}

#[derive(Clone)]
pub struct OneofWithContext<'a> {
    pub oneof: &'a OneofDescriptorProto,
    pub index: u32,
    pub message: MessageWithScope<'a>,
}

impl<'a> OneofWithContext<'a> {
    /// Oneof rust name
    pub fn name(&'a self) -> &'a str {
        match self.oneof.get_name() {
            "type" => "field_type",
            "box" => "field_box",
            x => x,
        }
    }

    /// rust type name of enum
    pub fn rust_name(&self) -> String {
        format!(
            "{}_oneof_{}",
            self.message.rust_name(),
            self.oneof.get_name()
        )
    }

    /// Oneof variants
    pub fn variants(&'a self) -> Vec<OneofVariantWithContext<'a>> {
        self.message
            .fields()
            .iter()
            .filter(|f| f.field.has_oneof_index() && f.field.get_oneof_index() == self.index as i32)
            .map(|f| OneofVariantWithContext {
                oneof: self,
                field: &f.field,
            })
            .collect()
    }
}

/// Find message by rust type name
pub fn find_message_by_rust_name<'a>(
    fd: &'a FileDescriptorProto,
    rust_name: &str,
) -> MessageWithScope<'a> {
    FileScope {
        file_descriptor: fd,
    }
    .find_messages()
    .into_iter()
    .find(|m| m.rust_name() == rust_name)
    .unwrap()
}

/// Find enum by rust type name
pub fn find_enum_by_rust_name<'a>(
    fd: &'a FileDescriptorProto,
    rust_name: &str,
) -> EnumWithScope<'a> {
    FileScope {
        file_descriptor: fd,
    }
    .find_enums()
    .into_iter()
    .find(|e| e.rust_name() == rust_name)
    .unwrap()
}

#[cfg(test)]
mod test {

    use super::proto_path_to_rust_mod;

    #[test]
    fn test_mod_path_proto_ext() {
        assert_eq!("proto", proto_path_to_rust_mod("proto.proto"));
    }

    #[test]
    fn test_mod_path_unknown_ext() {
        assert_eq!("proto_proto3", proto_path_to_rust_mod("proto.proto3"));
    }

    #[test]
    fn test_mod_path_empty_ext() {
        assert_eq!("proto", proto_path_to_rust_mod("proto"));
    }
}
