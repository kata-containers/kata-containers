use field::rust_field_name_for_protobuf_field_name;
use inside::protobuf_crate_path;
use protobuf::descriptor::*;
use protobuf_name::ProtobufAbsolutePath;
use scope::RootScope;
use Customize;

use super::code_writer::CodeWriter;
use super::rust_types_values::*;

struct ExtGen<'a> {
    file: &'a FileDescriptorProto,
    root_scope: &'a RootScope<'a>,
    field: &'a FieldDescriptorProto,
    customize: Customize,
}

impl<'a> ExtGen<'a> {
    fn extendee_rust_name(&self) -> String {
        type_name_to_rust_relative(
            &ProtobufAbsolutePath::from(self.field.get_extendee()),
            self.file,
            true,
            self.root_scope,
            &self.customize,
        )
    }

    fn repeated(&self) -> bool {
        match self.field.get_label() {
            FieldDescriptorProto_Label::LABEL_REPEATED => true,
            FieldDescriptorProto_Label::LABEL_OPTIONAL => false,
            FieldDescriptorProto_Label::LABEL_REQUIRED => {
                panic!("required ext field: {}", self.field.get_name())
            }
        }
    }

    fn return_type_gen(&self) -> ProtobufTypeGen {
        if self.field.has_type_name() {
            let rust_name_relative = type_name_to_rust_relative(
                &ProtobufAbsolutePath::from(self.field.get_type_name()),
                self.file,
                true,
                self.root_scope,
                &self.customize,
            );
            match self.field.get_field_type() {
                FieldDescriptorProto_Type::TYPE_MESSAGE => {
                    ProtobufTypeGen::Message(rust_name_relative)
                }
                FieldDescriptorProto_Type::TYPE_ENUM => ProtobufTypeGen::Enum(rust_name_relative),
                t => panic!("unknown type: {:?}", t),
            }
        } else {
            ProtobufTypeGen::Primitive(self.field.get_field_type(), PrimitiveTypeVariant::Default)
        }
    }

    fn write(&self, w: &mut CodeWriter) {
        let suffix = if self.repeated() {
            "Repeated"
        } else {
            "Optional"
        };
        let field_type = format!(
            "{}::ext::ExtField{}",
            protobuf_crate_path(&self.customize),
            suffix
        );
        w.pub_const(
            rust_field_name_for_protobuf_field_name(self.field.get_name()).get(),
            &format!(
                "{}<{}, {}>",
                field_type,
                self.extendee_rust_name(),
                self.return_type_gen().rust_type(&self.customize),
            ),
            &format!(
                "{} {{ field_number: {}, phantom: ::std::marker::PhantomData }}",
                field_type,
                self.field.get_number()
            ),
        );
    }
}

pub(crate) fn write_extensions(
    file: &FileDescriptorProto,
    root_scope: &RootScope,
    w: &mut CodeWriter,
    customize: &Customize,
) {
    if file.get_extension().is_empty() {
        return;
    }

    w.write_line("");
    w.write_line("/// Extension fields");
    w.pub_mod("exts", |w| {
        for field in file.get_extension() {
            if field.get_field_type() == FieldDescriptorProto_Type::TYPE_GROUP {
                continue;
            }

            w.write_line("");
            ExtGen {
                file: file,
                root_scope: root_scope,
                field: field,
                customize: customize.clone(),
            }
            .write(w);
        }
    });
}
