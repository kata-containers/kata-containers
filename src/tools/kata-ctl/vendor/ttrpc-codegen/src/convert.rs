//! Convert parser model to rust-protobuf model

use std::iter;

use crate::model;

use crate::str_lit::StrLitDecodeError;
use protobuf::Message;
use std::mem;

#[derive(Debug)]
pub enum ConvertError {
    UnsupportedOption(String),
    ExtensionNotFound(String),
    WrongExtensionType(String, &'static str),
    UnsupportedExtensionType(String, String),
    StrLitDecodeError(StrLitDecodeError),
    DefaultValueIsNotStringLiteral,
    WrongOptionType,
}

impl From<StrLitDecodeError> for ConvertError {
    fn from(e: StrLitDecodeError) -> Self {
        ConvertError::StrLitDecodeError(e)
    }
}

pub type ConvertResult<T> = Result<T, ConvertError>;

trait ProtobufOptions {
    fn by_name(&self, name: &str) -> Option<&model::ProtobufConstant>;

    fn by_name_bool(&self, name: &str) -> ConvertResult<Option<bool>> {
        match self.by_name(name) {
            Some(&model::ProtobufConstant::Bool(b)) => Ok(Some(b)),
            Some(_) => Err(ConvertError::WrongOptionType),
            None => Ok(None),
        }
    }
}

impl<'a> ProtobufOptions for &'a [model::ProtobufOption] {
    fn by_name(&self, name: &str) -> Option<&model::ProtobufConstant> {
        let option_name = name;
        for &model::ProtobufOption {
            ref name,
            ref value,
        } in *self
        {
            if name == option_name {
                return Some(value);
            }
        }
        None
    }
}

enum MessageOrEnum {
    Message,
    Enum,
}

impl MessageOrEnum {
    fn descriptor_type(&self) -> protobuf::descriptor::FieldDescriptorProto_Type {
        match *self {
            MessageOrEnum::Message => protobuf::descriptor::FieldDescriptorProto_Type::TYPE_MESSAGE,
            MessageOrEnum::Enum => protobuf::descriptor::FieldDescriptorProto_Type::TYPE_ENUM,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct RelativePath {
    path: String,
}

impl RelativePath {
    fn empty() -> RelativePath {
        RelativePath::new(String::new())
    }

    fn new(path: String) -> RelativePath {
        assert!(!path.starts_with("."));

        RelativePath { path }
    }

    fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    fn _last_part(&self) -> Option<&str> {
        match self.path.rfind('.') {
            Some(pos) => Some(&self.path[pos + 1..]),
            None => {
                if self.path.is_empty() {
                    None
                } else {
                    Some(&self.path)
                }
            }
        }
    }

    fn parent(&self) -> Option<RelativePath> {
        match self.path.rfind('.') {
            Some(pos) => Some(RelativePath::new(self.path[..pos].to_owned())),
            None => {
                if self.path.is_empty() {
                    None
                } else {
                    Some(RelativePath::empty())
                }
            }
        }
    }

    fn self_and_parents(&self) -> Vec<RelativePath> {
        let mut tmp = self.clone();

        let mut r = Vec::new();

        r.push(self.clone());

        while let Some(parent) = tmp.parent() {
            r.push(parent.clone());
            tmp = parent;
        }

        r
    }

    fn append(&self, simple: &str) -> RelativePath {
        if self.path.is_empty() {
            RelativePath::new(simple.to_owned())
        } else {
            RelativePath::new(format!("{}.{}", self.path, simple))
        }
    }

    fn split_first_rem(&self) -> Option<(&str, RelativePath)> {
        if self.is_empty() {
            None
        } else {
            Some(match self.path.find('.') {
                Some(dot) => (
                    &self.path[..dot],
                    RelativePath::new(self.path[dot + 1..].to_owned()),
                ),
                None => (&self.path, RelativePath::empty()),
            })
        }
    }
}

#[cfg(test)]
mod relative_path_test {
    use super::*;

    #[test]
    fn parent() {
        assert_eq!(None, RelativePath::empty().parent());
        assert_eq!(
            Some(RelativePath::empty()),
            RelativePath::new("aaa".to_owned()).parent()
        );
        assert_eq!(
            Some(RelativePath::new("abc".to_owned())),
            RelativePath::new("abc.def".to_owned()).parent()
        );
        assert_eq!(
            Some(RelativePath::new("abc.def".to_owned())),
            RelativePath::new("abc.def.gh".to_owned()).parent()
        );
    }

    #[test]
    fn last_part() {
        assert_eq!(None, RelativePath::empty()._last_part());
        assert_eq!(
            Some("aaa"),
            RelativePath::new("aaa".to_owned())._last_part()
        );
        assert_eq!(
            Some("def"),
            RelativePath::new("abc.def".to_owned())._last_part()
        );
        assert_eq!(
            Some("gh"),
            RelativePath::new("abc.def.gh".to_owned())._last_part()
        );
    }

    #[test]
    fn self_and_parents() {
        assert_eq!(
            vec![
                RelativePath::new("ab.cde.fghi".to_owned()),
                RelativePath::new("ab.cde".to_owned()),
                RelativePath::new("ab".to_owned()),
                RelativePath::empty(),
            ],
            RelativePath::new("ab.cde.fghi".to_owned()).self_and_parents()
        );
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
struct AbsolutePath {
    path: String,
}

impl AbsolutePath {
    fn root() -> AbsolutePath {
        AbsolutePath::new(String::new())
    }

    fn new(path: String) -> AbsolutePath {
        assert!(path.is_empty() || path.starts_with("."));
        assert!(!path.ends_with("."));
        AbsolutePath { path }
    }

    fn from_path_without_dot(path: &str) -> AbsolutePath {
        if path.is_empty() {
            AbsolutePath::root()
        } else {
            assert!(!path.starts_with("."));
            assert!(!path.ends_with("."));
            AbsolutePath::new(format!(".{}", path))
        }
    }

    fn from_path_maybe_dot(path: &str) -> AbsolutePath {
        if path.starts_with(".") {
            AbsolutePath::new(path.to_owned())
        } else {
            AbsolutePath::from_path_without_dot(path)
        }
    }

    fn push_simple(&mut self, simple: &str) {
        assert!(!simple.is_empty());
        assert!(!simple.contains('.'));
        self.path.push('.');
        self.path.push_str(simple);
    }

    fn push_relative(&mut self, relative: &RelativePath) {
        if !relative.is_empty() {
            self.path.push('.');
            self.path.push_str(&relative.path);
        }
    }

    fn remove_prefix(&self, prefix: &AbsolutePath) -> Option<RelativePath> {
        if self.path.starts_with(&prefix.path) {
            let rem = &self.path[prefix.path.len()..];
            if rem.is_empty() {
                return Some(RelativePath::empty());
            }
            if rem.starts_with('.') {
                return Some(RelativePath::new(rem[1..].to_owned()));
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn absolute_path_push_simple() {
        let mut foo = AbsolutePath::new(".foo".to_owned());
        foo.push_simple("bar");
        assert_eq!(AbsolutePath::new(".foo.bar".to_owned()), foo);

        let mut foo = AbsolutePath::root();
        foo.push_simple("bar");
        assert_eq!(AbsolutePath::new(".bar".to_owned()), foo);
    }

    #[test]
    fn absolute_path_remove_prefix() {
        assert_eq!(
            Some(RelativePath::empty()),
            AbsolutePath::new(".foo".to_owned())
                .remove_prefix(&AbsolutePath::new(".foo".to_owned()))
        );
        assert_eq!(
            Some(RelativePath::new("bar".to_owned())),
            AbsolutePath::new(".foo.bar".to_owned())
                .remove_prefix(&AbsolutePath::new(".foo".to_owned()))
        );
        assert_eq!(
            Some(RelativePath::new("baz.qux".to_owned())),
            AbsolutePath::new(".foo.bar.baz.qux".to_owned())
                .remove_prefix(&AbsolutePath::new(".foo.bar".to_owned()))
        );
        assert_eq!(
            None,
            AbsolutePath::new(".foo.barbaz".to_owned())
                .remove_prefix(&AbsolutePath::new(".foo.bar".to_owned()))
        );
    }
}

enum LookupScope<'a> {
    File(&'a model::FileDescriptor),
    Message(&'a model::Message),
}

impl<'a> LookupScope<'a> {
    fn messages(&self) -> &[model::Message] {
        match self {
            &LookupScope::File(file) => &file.messages,
            &LookupScope::Message(messasge) => &messasge.messages,
        }
    }

    fn find_message(&self, simple_name: &str) -> Option<&model::Message> {
        self.messages().into_iter().find(|m| m.name == simple_name)
    }

    fn enums(&self) -> &[model::Enumeration] {
        match self {
            &LookupScope::File(file) => &file.enums,
            &LookupScope::Message(messasge) => &messasge.enums,
        }
    }

    fn members(&self) -> Vec<(&str, MessageOrEnum)> {
        let mut r = Vec::new();
        r.extend(
            self.enums()
                .into_iter()
                .map(|e| (&e.name[..], MessageOrEnum::Enum)),
        );
        r.extend(
            self.messages()
                .into_iter()
                .map(|e| (&e.name[..], MessageOrEnum::Message)),
        );
        r
    }

    fn find_member(&self, simple_name: &str) -> Option<MessageOrEnum> {
        self.members()
            .into_iter()
            .filter_map(|(member_name, message_or_enum)| {
                if member_name == simple_name {
                    Some(message_or_enum)
                } else {
                    None
                }
            })
            .next()
    }

    fn resolve_message_or_enum(
        &self,
        current_path: &AbsolutePath,
        path: &RelativePath,
    ) -> Option<(AbsolutePath, MessageOrEnum)> {
        let (first, rem) = match path.split_first_rem() {
            Some(x) => x,
            None => return None,
        };

        if rem.is_empty() {
            match self.find_member(&first) {
                Some(message_or_enum) => {
                    let mut result_path = current_path.clone();
                    result_path.push_simple(&first);
                    Some((result_path, message_or_enum))
                }
                None => None,
            }
        } else {
            match self.find_message(&first) {
                Some(message) => {
                    let mut message_path = current_path.clone();
                    message_path.push_simple(&message.name);
                    let message_scope = LookupScope::Message(message);
                    message_scope.resolve_message_or_enum(&message_path, &rem)
                }
                None => None,
            }
        }
    }
}

struct Resolver<'a> {
    current_file: &'a model::FileDescriptor,
    deps: &'a [model::FileDescriptor],
}

impl<'a> Resolver<'a> {
    fn map_entry_name_for_field_name(field_name: &str) -> String {
        format!("{}_MapEntry", field_name)
    }

    fn map_entry_field(
        &self,
        name: &str,
        number: i32,
        field_type: &model::FieldType,
        path_in_file: &RelativePath,
    ) -> protobuf::descriptor::FieldDescriptorProto {
        let mut output = protobuf::descriptor::FieldDescriptorProto::new();
        output.set_name(name.to_owned());
        output.set_number(number);

        let (t, t_name) = self.field_type(name, field_type, path_in_file);
        output.set_field_type(t);
        if let Some(t_name) = t_name {
            output.set_type_name(t_name.path);
        }

        output
    }

    fn map_entry_message(
        &self,
        field_name: &str,
        key: &model::FieldType,
        value: &model::FieldType,
        path_in_file: &RelativePath,
    ) -> ConvertResult<protobuf::descriptor::DescriptorProto> {
        let mut output = protobuf::descriptor::DescriptorProto::new();

        output.mut_options().set_map_entry(true);
        output.set_name(Resolver::map_entry_name_for_field_name(field_name));
        output
            .mut_field()
            .push(self.map_entry_field("key", 1, key, path_in_file));
        output
            .mut_field()
            .push(self.map_entry_field("value", 2, value, path_in_file));

        Ok(output)
    }

    fn message_options(
        &self,
        input: &[model::ProtobufOption],
    ) -> ConvertResult<protobuf::descriptor::MessageOptions> {
        let mut r = protobuf::descriptor::MessageOptions::new();
        self.custom_options(
            input,
            "google.protobuf.MessageOptions",
            r.mut_unknown_fields(),
        )?;
        Ok(r)
    }

    fn message(
        &self,
        input: &model::Message,
        path_in_file: &RelativePath,
    ) -> ConvertResult<protobuf::descriptor::DescriptorProto> {
        let nested_path_in_file = path_in_file.append(&input.name);

        let mut output = protobuf::descriptor::DescriptorProto::new();
        output.set_name(input.name.clone());

        let mut nested_messages = protobuf::RepeatedField::new();

        for m in &input.messages {
            nested_messages.push(self.message(m, &nested_path_in_file)?);
        }

        for f in &input.fields {
            if let model::FieldType::Map(ref t) = f.typ {
                nested_messages.push(self.map_entry_message(&f.name, &t.0, &t.1, path_in_file)?);
            }
        }

        output.set_nested_type(nested_messages);

        output.set_enum_type(
            input
                .enums
                .iter()
                .map(|e| self.enumeration(e))
                .collect::<Result<_, _>>()?,
        );

        {
            let mut fields = protobuf::RepeatedField::new();

            for f in &input.fields {
                fields.push(self.field(f, None, &nested_path_in_file)?);
            }

            for (oneof_index, oneof) in input.oneofs.iter().enumerate() {
                let oneof_index = oneof_index as i32;
                for f in &oneof.fields {
                    fields.push(self.field(f, Some(oneof_index as i32), &nested_path_in_file)?);
                }
            }

            output.set_field(fields);
        }

        let oneofs = input.oneofs.iter().map(|o| self.oneof(o)).collect();
        output.set_oneof_decl(oneofs);

        output.set_options(self.message_options(&input.options)?);

        Ok(output)
    }

    fn service_options(
        &self,
        input: &[model::ProtobufOption],
    ) -> ConvertResult<protobuf::descriptor::ServiceOptions> {
        let mut r = protobuf::descriptor::ServiceOptions::new();
        self.custom_options(
            input,
            "google.protobuf.ServiceOptions",
            r.mut_unknown_fields(),
        )?;
        Ok(r)
    }

    fn method_options(
        &self,
        input: &[model::ProtobufOption],
    ) -> ConvertResult<protobuf::descriptor::MethodOptions> {
        let mut r = protobuf::descriptor::MethodOptions::new();
        self.custom_options(
            input,
            "google.protobuf.MethodOptions",
            r.mut_unknown_fields(),
        )?;
        Ok(r)
    }

    fn service(
        &self,
        input: &model::Service,
        package: &String,
    ) -> ConvertResult<protobuf::descriptor::ServiceDescriptorProto> {
        let mut output = protobuf::descriptor::ServiceDescriptorProto::new();
        output.set_name(input.name.clone());

        let mut methods = protobuf::RepeatedField::new();
        for m in &input.methods {
            let mut mm = protobuf::descriptor::MethodDescriptorProto::new();
            mm.set_name(m.name.clone());

            mm.set_input_type(to_protobuf_absolute_path(&package, m.input_type.clone()));
            mm.set_output_type(to_protobuf_absolute_path(&package, m.output_type.clone()));

            mm.set_client_streaming(m.client_streaming);
            mm.set_server_streaming(m.server_streaming);
            mm.set_options(self.method_options(&m.options)?);

            methods.push(mm);
        }

        output.set_method(methods);
        output.set_options(self.service_options(&input.options)?);

        Ok(output)
    }

    fn custom_options(
        &self,
        input: &[model::ProtobufOption],
        extendee: &'static str,
        unknown_fields: &mut protobuf::UnknownFields,
    ) -> ConvertResult<()> {
        for option in input {
            // TODO: builtin options too
            if !option.name.starts_with('(') {
                continue;
            }

            let extension = match self.find_extension(&option.name) {
                Ok(e) => e,
                // TODO: return error
                Err(_) => continue,
            };
            if extension.extendee != extendee {
                return Err(ConvertError::WrongExtensionType(
                    option.name.clone(),
                    extendee,
                ));
            }

            let value = match Resolver::option_value_to_unknown_value(
                &option.value,
                &extension.field.typ,
                &option.name,
            ) {
                Ok(value) => value,
                Err(_) => {
                    // TODO: return error
                    continue;
                }
            };

            unknown_fields.add_value(extension.field.number as u32, value);
        }
        Ok(())
    }

    fn field_options(
        &self,
        input: &[model::ProtobufOption],
    ) -> ConvertResult<protobuf::descriptor::FieldOptions> {
        let mut r = protobuf::descriptor::FieldOptions::new();
        if let Some(deprecated) = input.by_name_bool("deprecated")? {
            r.set_deprecated(deprecated);
        }
        if let Some(packed) = input.by_name_bool("packed")? {
            r.set_packed(packed);
        }
        self.custom_options(
            input,
            "google.protobuf.FieldOptions",
            r.mut_unknown_fields(),
        )?;
        Ok(r)
    }

    fn field(
        &self,
        input: &model::Field,
        oneof_index: Option<i32>,
        path_in_file: &RelativePath,
    ) -> ConvertResult<protobuf::descriptor::FieldDescriptorProto> {
        let mut output = protobuf::descriptor::FieldDescriptorProto::new();
        output.set_name(input.name.clone());

        if let model::FieldType::Map(..) = input.typ {
            output.set_label(protobuf::descriptor::FieldDescriptorProto_Label::LABEL_REPEATED);
        } else {
            output.set_label(label(input.rule));
        }

        let (t, t_name) = self.field_type(&input.name, &input.typ, path_in_file);
        output.set_field_type(t);
        if let Some(t_name) = t_name {
            output.set_type_name(t_name.path);
        }

        output.set_number(input.number);
        if let Some(default) = input.options.as_slice().by_name("default") {
            let default = match output.get_field_type() {
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_STRING => {
                    if let &model::ProtobufConstant::String(ref s) = default {
                        s.decode_utf8()?
                    } else {
                        return Err(ConvertError::DefaultValueIsNotStringLiteral);
                    }
                }
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_BYTES => {
                    if let &model::ProtobufConstant::String(ref s) = default {
                        s.escaped.clone()
                    } else {
                        return Err(ConvertError::DefaultValueIsNotStringLiteral);
                    }
                }
                _ => default.format(),
            };
            output.set_default_value(default);
        }

        output.set_options(self.field_options(&input.options)?);

        if let Some(oneof_index) = oneof_index {
            output.set_oneof_index(oneof_index);
        }

        Ok(output)
    }

    fn all_files(&self) -> Vec<&model::FileDescriptor> {
        iter::once(self.current_file).chain(self.deps).collect()
    }

    fn package_files(&self, package: &str) -> Vec<&model::FileDescriptor> {
        self.all_files()
            .into_iter()
            .filter(|f| f.package == package)
            .collect()
    }

    fn current_file_package_files(&self) -> Vec<&model::FileDescriptor> {
        self.package_files(&self.current_file.package)
    }

    fn resolve_message_or_enum(
        &self,
        name: &str,
        path_in_file: &RelativePath,
    ) -> (AbsolutePath, MessageOrEnum) {
        // find message or enum in current package
        if !name.starts_with(".") {
            for p in path_in_file.self_and_parents() {
                let relative_path_with_name = p.clone();
                let relative_path_with_name = relative_path_with_name.append(name);
                for file in self.current_file_package_files() {
                    if let Some((n, t)) = LookupScope::File(file).resolve_message_or_enum(
                        &AbsolutePath::from_path_without_dot(&file.package),
                        &relative_path_with_name,
                    ) {
                        return (n, t);
                    }
                }
            }
        }

        // find message or enum in root package
        {
            let absolute_path = AbsolutePath::from_path_maybe_dot(name);
            for file in self.all_files() {
                let file_package = AbsolutePath::from_path_without_dot(&file.package);
                if let Some(relative) = absolute_path.remove_prefix(&file_package) {
                    if let Some((n, t)) =
                        LookupScope::File(file).resolve_message_or_enum(&file_package, &relative)
                    {
                        return (n, t);
                    }
                }
            }
        }

        panic!(
            "couldn't find message or enum {} when parsing {}",
            name, self.current_file.package
        );
    }

    fn field_type(
        &self,
        name: &str,
        input: &model::FieldType,
        path_in_file: &RelativePath,
    ) -> (
        protobuf::descriptor::FieldDescriptorProto_Type,
        Option<AbsolutePath>,
    ) {
        match *input {
            model::FieldType::Bool => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_BOOL,
                None,
            ),
            model::FieldType::Int32 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_INT32,
                None,
            ),
            model::FieldType::Int64 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_INT64,
                None,
            ),
            model::FieldType::Uint32 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_UINT32,
                None,
            ),
            model::FieldType::Uint64 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_UINT64,
                None,
            ),
            model::FieldType::Sint32 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_SINT32,
                None,
            ),
            model::FieldType::Sint64 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_SINT64,
                None,
            ),
            model::FieldType::Fixed32 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_FIXED32,
                None,
            ),
            model::FieldType::Fixed64 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_FIXED64,
                None,
            ),
            model::FieldType::Sfixed32 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_SFIXED32,
                None,
            ),
            model::FieldType::Sfixed64 => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_SFIXED64,
                None,
            ),
            model::FieldType::Float => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_FLOAT,
                None,
            ),
            model::FieldType::Double => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_DOUBLE,
                None,
            ),
            model::FieldType::String => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_STRING,
                None,
            ),
            model::FieldType::Bytes => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_BYTES,
                None,
            ),
            model::FieldType::MessageOrEnum(ref name) => {
                let (name, me) = self.resolve_message_or_enum(&name, path_in_file);
                (me.descriptor_type(), Some(name))
            }
            model::FieldType::Map(..) => {
                let mut type_name = AbsolutePath::from_path_without_dot(&self.current_file.package);
                type_name.push_relative(path_in_file);
                type_name.push_simple(&Resolver::map_entry_name_for_field_name(name));
                (
                    protobuf::descriptor::FieldDescriptorProto_Type::TYPE_MESSAGE,
                    Some(type_name),
                )
            }
            model::FieldType::Group(..) => (
                protobuf::descriptor::FieldDescriptorProto_Type::TYPE_GROUP,
                None,
            ),
        }
    }

    fn enum_value(
        &self,
        name: &str,
        number: i32,
    ) -> protobuf::descriptor::EnumValueDescriptorProto {
        let mut output = protobuf::descriptor::EnumValueDescriptorProto::new();
        output.set_name(name.to_owned());
        output.set_number(number);
        output
    }

    fn enum_options(
        &self,
        input: &[model::ProtobufOption],
    ) -> ConvertResult<protobuf::descriptor::EnumOptions> {
        let mut r = protobuf::descriptor::EnumOptions::new();
        if let Some(allow_alias) = input.by_name_bool("allow_alias")? {
            r.set_allow_alias(allow_alias);
        }
        if let Some(deprecated) = input.by_name_bool("deprecated")? {
            r.set_deprecated(deprecated);
        }
        self.custom_options(input, "google.protobuf.EnumOptions", r.mut_unknown_fields())?;
        Ok(r)
    }

    fn enumeration(
        &self,
        input: &model::Enumeration,
    ) -> ConvertResult<protobuf::descriptor::EnumDescriptorProto> {
        let mut output = protobuf::descriptor::EnumDescriptorProto::new();
        output.set_name(input.name.clone());
        output.set_value(
            input
                .values
                .iter()
                .map(|v| self.enum_value(&v.name, v.number))
                .collect(),
        );
        output.set_options(self.enum_options(&input.options)?);
        Ok(output)
    }

    fn oneof(&self, input: &model::OneOf) -> protobuf::descriptor::OneofDescriptorProto {
        let mut output = protobuf::descriptor::OneofDescriptorProto::new();
        output.set_name(input.name.clone());
        output
    }

    fn find_extension_by_path(&self, path: &str) -> ConvertResult<&model::Extension> {
        let (package, name) = match path.rfind('.') {
            Some(dot) => (&path[..dot], &path[dot + 1..]),
            None => (self.current_file.package.as_str(), path),
        };

        for file in self.package_files(package) {
            for ext in &file.extensions {
                if ext.field.name == name {
                    return Ok(ext);
                }
            }
        }

        Err(ConvertError::ExtensionNotFound(path.to_owned()))
    }

    fn find_extension(&self, option_name: &str) -> ConvertResult<&model::Extension> {
        if !option_name.starts_with('(') || !option_name.ends_with(')') {
            return Err(ConvertError::UnsupportedOption(option_name.to_owned()));
        }
        let path = &option_name[1..option_name.len() - 1];
        self.find_extension_by_path(path)
    }

    fn option_value_to_unknown_value(
        value: &model::ProtobufConstant,
        field_type: &model::FieldType,
        option_name: &str,
    ) -> ConvertResult<protobuf::UnknownValue> {
        let v = match value {
            &model::ProtobufConstant::Bool(b) => {
                if field_type != &model::FieldType::Bool {
                    Err(())
                } else {
                    Ok(protobuf::UnknownValue::Varint(if b { 1 } else { 0 }))
                }
            }
            // TODO: check overflow
            &model::ProtobufConstant::U64(v) => match field_type {
                &model::FieldType::Fixed64 | &model::FieldType::Sfixed64 => {
                    Ok(protobuf::UnknownValue::Fixed64(v))
                }
                &model::FieldType::Fixed32 | &model::FieldType::Sfixed32 => {
                    Ok(protobuf::UnknownValue::Fixed32(v as u32))
                }
                &model::FieldType::Int64
                | &model::FieldType::Int32
                | &model::FieldType::Uint64
                | &model::FieldType::Uint32 => Ok(protobuf::UnknownValue::Varint(v)),
                &model::FieldType::Sint64 => Ok(protobuf::UnknownValue::sint64(v as i64)),
                &model::FieldType::Sint32 => Ok(protobuf::UnknownValue::sint32(v as i32)),
                _ => Err(()),
            },
            &model::ProtobufConstant::I64(v) => match field_type {
                &model::FieldType::Fixed64 | &model::FieldType::Sfixed64 => {
                    Ok(protobuf::UnknownValue::Fixed64(v as u64))
                }
                &model::FieldType::Fixed32 | &model::FieldType::Sfixed32 => {
                    Ok(protobuf::UnknownValue::Fixed32(v as u32))
                }
                &model::FieldType::Int64
                | &model::FieldType::Int32
                | &model::FieldType::Uint64
                | &model::FieldType::Uint32 => Ok(protobuf::UnknownValue::Varint(v as u64)),
                &model::FieldType::Sint64 => Ok(protobuf::UnknownValue::sint64(v as i64)),
                &model::FieldType::Sint32 => Ok(protobuf::UnknownValue::sint32(v as i32)),
                _ => Err(()),
            },
            &model::ProtobufConstant::F64(f) => match field_type {
                &model::FieldType::Float => Ok(protobuf::UnknownValue::Fixed32(unsafe {
                    mem::transmute::<f32, u32>(f as f32)
                })),
                &model::FieldType::Double => Ok(protobuf::UnknownValue::Fixed64(unsafe {
                    mem::transmute::<f64, u64>(f)
                })),
                _ => Err(()),
            },
            &model::ProtobufConstant::String(ref s) => {
                match field_type {
                    &model::FieldType::String => Ok(protobuf::UnknownValue::LengthDelimited(
                        s.decode_utf8()?.into_bytes(),
                    )),
                    // TODO: bytes
                    _ => Err(()),
                }
            }
            _ => Err(()),
        };

        v.map_err(|()| {
            ConvertError::UnsupportedExtensionType(
                option_name.to_owned(),
                format!("{:?}", field_type),
            )
        })
    }

    fn file_options(
        &self,
        input: &[model::ProtobufOption],
    ) -> ConvertResult<protobuf::descriptor::FileOptions> {
        let mut r = protobuf::descriptor::FileOptions::new();
        self.custom_options(input, "google.protobuf.FileOptions", r.mut_unknown_fields())?;
        Ok(r)
    }

    fn extension(
        &self,
        input: &model::Extension,
    ) -> ConvertResult<protobuf::descriptor::FieldDescriptorProto> {
        let relative_path = RelativePath::new("".to_owned());
        let mut field = self.field(&input.field, None, &relative_path)?;
        field.set_extendee(
            self.resolve_message_or_enum(&input.extendee, &relative_path)
                .0
                .path,
        );
        Ok(field)
    }
}

fn to_protobuf_absolute_path(package: &String, path: String) -> String {
    if !path.starts_with(".") {
        if path.contains(".") {
            return format!(".{}", &path);
        } else {
            return format!(".{}.{}", package, &path);
        }
    }

    path
}

fn syntax(input: model::Syntax) -> String {
    match input {
        model::Syntax::Proto2 => "proto2".to_owned(),
        model::Syntax::Proto3 => "proto3".to_owned(),
    }
}

fn label(input: model::Rule) -> protobuf::descriptor::FieldDescriptorProto_Label {
    match input {
        model::Rule::Optional => protobuf::descriptor::FieldDescriptorProto_Label::LABEL_OPTIONAL,
        model::Rule::Required => protobuf::descriptor::FieldDescriptorProto_Label::LABEL_REQUIRED,
        model::Rule::Repeated => protobuf::descriptor::FieldDescriptorProto_Label::LABEL_REPEATED,
    }
}

pub fn file_descriptor(
    name: String,
    input: &model::FileDescriptor,
    deps: &[model::FileDescriptor],
) -> ConvertResult<protobuf::descriptor::FileDescriptorProto> {
    let resolver = Resolver {
        current_file: &input,
        deps,
    };

    let mut output = protobuf::descriptor::FileDescriptorProto::new();
    output.set_name(name);
    output.set_package(input.package.clone());
    output.set_syntax(syntax(input.syntax));

    let mut messages = protobuf::RepeatedField::new();
    for m in &input.messages {
        messages.push(resolver.message(&m, &RelativePath::empty())?);
    }

    output.set_message_type(messages);

    let mut services = protobuf::RepeatedField::new();
    for s in &input.services {
        services.push(resolver.service(&s, &input.package)?);
    }

    output.set_service(services);

    output.set_enum_type(
        input
            .enums
            .iter()
            .map(|e| resolver.enumeration(e))
            .collect::<Result<_, _>>()?,
    );

    output.set_options(resolver.file_options(&input.options)?);

    let mut extensions = protobuf::RepeatedField::new();
    for e in &input.extensions {
        extensions.push(resolver.extension(e)?);
    }
    output.set_extension(extensions);

    Ok(output)
}
