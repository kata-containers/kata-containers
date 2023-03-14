use std::collections::HashSet;

use file_descriptor::file_descriptor_proto_expr;
use inside::protobuf_crate_path;
use protobuf::descriptor::*;
use protobuf_name::ProtobufAbsolutePath;
use rust_types_values::type_name_to_rust_relative;
use scope::EnumWithScope;
use scope::RootScope;
use scope::WithScope;
use serde;
use CodeWriter;

use crate::customize::customize_from_rustproto_for_enum;
use crate::Customize;

#[derive(Clone)]
pub struct EnumValueGen {
    proto: EnumValueDescriptorProto,
    enum_rust_name: String,
    variant_rust_name: String,
}

impl EnumValueGen {
    fn parse(
        proto: &EnumValueDescriptorProto,
        enum_rust_name: &str,
        variant_rust_name: &str,
    ) -> EnumValueGen {
        EnumValueGen {
            proto: proto.clone(),
            enum_rust_name: enum_rust_name.to_string(),
            variant_rust_name: variant_rust_name.to_string(),
        }
    }

    // enum value
    fn number(&self) -> i32 {
        self.proto.get_number()
    }

    // name of enum variant in generated rust code
    fn rust_name_inner(&self) -> String {
        self.variant_rust_name.clone()
    }

    pub fn rust_name_outer(&self) -> String {
        let mut r = String::new();
        r.push_str(&self.enum_rust_name);
        r.push_str("::");
        r.push_str(&self.rust_name_inner());
        r
    }
}

pub(crate) struct EnumGen<'a> {
    enum_with_scope: &'a EnumWithScope<'a>,
    type_name: String,
    lite_runtime: bool,
    customize: Customize,
}

impl<'a> EnumGen<'a> {
    pub fn new(
        enum_with_scope: &'a EnumWithScope<'a>,
        current_file: &FileDescriptorProto,
        customize: &Customize,
        root_scope: &RootScope,
    ) -> EnumGen<'a> {
        let rust_name = if enum_with_scope.get_scope().get_file_descriptor().get_name()
            == current_file.get_name()
        {
            // field type is a message or enum declared in the same file
            enum_with_scope.rust_name().to_string()
        } else {
            type_name_to_rust_relative(
                &ProtobufAbsolutePath::from(enum_with_scope.name_absolute()),
                current_file,
                false,
                root_scope,
                customize,
            )
            .to_string()
        };
        let lite_runtime = customize.lite_runtime.unwrap_or_else(|| {
            enum_with_scope
                .get_scope()
                .get_file_descriptor()
                .get_options()
                .get_optimize_for()
                == FileOptions_OptimizeMode::LITE_RUNTIME
        });

        let mut customize = customize.clone();
        customize.update_with(&customize_from_rustproto_for_enum(
            enum_with_scope.en.options.as_ref().unwrap_or_default(),
        ));

        EnumGen {
            enum_with_scope,
            type_name: rust_name,
            lite_runtime,
            customize,
        }
    }

    fn allow_alias(&self) -> bool {
        self.enum_with_scope.en.get_options().get_allow_alias()
    }

    fn values_all(&self) -> Vec<EnumValueGen> {
        let mut r = Vec::new();
        for p in self.enum_with_scope.values() {
            r.push(EnumValueGen::parse(
                &p.proto,
                &self.type_name,
                p.rust_name().get(),
            ));
        }
        r
    }

    pub fn values_unique(&self) -> Vec<EnumValueGen> {
        let mut used = HashSet::new();
        let mut r = Vec::new();
        for p in self.enum_with_scope.values() {
            // skipping non-unique enums
            // TODO: should support it
            if !used.insert(p.proto.get_number()) {
                continue;
            }
            r.push(EnumValueGen::parse(
                p.proto,
                &self.type_name,
                p.rust_name().get(),
            ));
        }
        r
    }

    // find enum value by name
    pub fn value_by_name(&'a self, name: &str) -> EnumValueGen {
        let v = self.enum_with_scope.value_by_name(name);
        EnumValueGen::parse(v.proto, &self.type_name, v.rust_name().get())
    }

    pub fn write(&self, w: &mut CodeWriter) {
        self.write_struct(w);
        if self.allow_alias() {
            w.write_line("");
            self.write_impl_eq(w);
            w.write_line("");
            self.write_impl_hash(w);
        }
        w.write_line("");
        self.write_impl_enum(w);
        w.write_line("");
        self.write_impl_copy(w);
        w.write_line("");
        self.write_impl_default(w);
        w.write_line("");
        self.write_impl_value(w);
    }

    fn write_struct(&self, w: &mut CodeWriter) {
        let mut derive = Vec::new();
        derive.push("Clone");
        if !self.allow_alias() {
            derive.push("PartialEq");
        }
        derive.push("Eq");
        derive.push("Debug");
        if !self.allow_alias() {
            derive.push("Hash");
        } else {
            w.comment("Note: you cannot use pattern matching for enums with allow_alias option");
        }
        w.derive(&derive);
        serde::write_serde_attr(
            w,
            &self.customize,
            "derive(::serde::Serialize, ::serde::Deserialize)",
        );
        if let Some(ref ren) = self.customize.serde_rename_all {
            let attr = format!("serde(rename_all = \"{}\")", ren);
            serde::write_serde_attr(w, &self.customize, &attr);
        }
        let ref type_name = self.type_name;
        w.expr_block(&format!("pub enum {}", type_name), |w| {
            for value in self.values_all() {
                if self.allow_alias() {
                    w.write_line(&format!(
                        "{}, // {}",
                        value.rust_name_inner(),
                        value.number()
                    ));
                } else {
                    w.write_line(&format!(
                        "{} = {},",
                        value.rust_name_inner(),
                        value.number()
                    ));
                }
            }
        });
    }

    fn write_fn_value(&self, w: &mut CodeWriter) {
        w.def_fn("value(&self) -> i32", |w| {
            if self.allow_alias() {
                w.match_expr("*self", |w| {
                    for value in self.values_all() {
                        w.case_expr(value.rust_name_outer(), format!("{}", value.number()));
                    }
                });
            } else {
                w.write_line("*self as i32")
            }
        });
    }

    fn write_impl_enum(&self, w: &mut CodeWriter) {
        let ref type_name = self.type_name;
        w.impl_for_block(
            &format!("{}::ProtobufEnum", protobuf_crate_path(&self.customize)),
            &format!("{}", type_name),
            |w| {
                self.write_fn_value(w);

                w.write_line("");
                let ref type_name = self.type_name;
                w.def_fn(
                    &format!(
                        "from_i32(value: i32) -> ::std::option::Option<{}>",
                        type_name
                    ),
                    |w| {
                        w.match_expr("value", |w| {
                            let values = self.values_unique();
                            for value in values {
                                w.write_line(&format!(
                                    "{} => ::std::option::Option::Some({}),",
                                    value.number(),
                                    value.rust_name_outer()
                                ));
                            }
                            w.write_line(&format!("_ => ::std::option::Option::None"));
                        });
                    },
                );

                w.write_line("");
                w.def_fn(&format!("values() -> &'static [Self]"), |w| {
                    w.write_line(&format!("static values: &'static [{}] = &[", type_name));
                    w.indented(|w| {
                        for value in self.values_all() {
                            w.write_line(&format!("{},", value.rust_name_outer()));
                        }
                    });
                    w.write_line("];");
                    w.write_line("values");
                });

                if !self.lite_runtime {
                    w.write_line("");
                    w.def_fn(
                        &format!(
                            "enum_descriptor_static() -> &'static {}::reflect::EnumDescriptor",
                            protobuf_crate_path(&self.customize)
                        ),
                        |w| {
                            w.lazy_static_decl_get(
                                "descriptor",
                                &format!(
                                    "{}::reflect::EnumDescriptor",
                                    protobuf_crate_path(&self.customize)
                                ),
                                &self.customize,
                                |w| {
                                    let ref type_name = self.type_name;
                                    w.write_line(&format!(
                                    "{}::reflect::EnumDescriptor::new_pb_name::<{}>(\"{}\", {})",
                                    protobuf_crate_path(&self.customize),
                                    type_name,
                                    self.enum_with_scope.name_to_package(),
                                    file_descriptor_proto_expr(&self.enum_with_scope.scope)
                                ));
                                },
                            );
                        },
                    );
                }
            },
        );
    }

    fn write_impl_value(&self, w: &mut CodeWriter) {
        w.impl_for_block(
            &format!(
                "{}::reflect::ProtobufValue",
                protobuf_crate_path(&self.customize)
            ),
            &format!("{}", self.type_name),
            |w| {
                w.def_fn(
                    &format!(
                        "as_ref(&self) -> {}::reflect::ReflectValueRef",
                        protobuf_crate_path(&self.customize)
                    ),
                    |w| {
                        w.write_line(&format!(
                        "{}::reflect::ReflectValueRef::Enum({}::ProtobufEnum::descriptor(self))",
                        protobuf_crate_path(&self.customize),
                        protobuf_crate_path(&self.customize)
                    ))
                    },
                )
            },
        )
    }

    fn write_impl_copy(&self, w: &mut CodeWriter) {
        w.impl_for_block("::std::marker::Copy", &self.type_name, |_w| {});
    }

    fn write_impl_eq(&self, w: &mut CodeWriter) {
        assert!(self.allow_alias());
        w.impl_for_block(
            "::std::cmp::PartialEq",
            &format!("{}", self.type_name),
            |w| {
                w.def_fn("eq(&self, other: &Self) -> bool", |w| {
                    w.write_line(&format!(
                        "{}::ProtobufEnum::value(self) == {}::ProtobufEnum::value(other)",
                        protobuf_crate_path(&self.customize),
                        protobuf_crate_path(&self.customize)
                    ));
                });
            },
        );
    }

    fn write_impl_hash(&self, w: &mut CodeWriter) {
        assert!(self.allow_alias());
        w.impl_for_block("::std::hash::Hash", &format!("{}", self.type_name), |w| {
            w.def_fn("hash<H : ::std::hash::Hasher>(&self, state: &mut H)", |w| {
                w.write_line(&format!(
                    "state.write_i32({}::ProtobufEnum::value(self))",
                    protobuf_crate_path(&self.customize)
                ));
            });
        });
    }

    fn write_impl_default(&self, w: &mut CodeWriter) {
        let first_value = &self.enum_with_scope.values()[0];
        if first_value.proto.get_number() != 0 {
            // This warning is emitted only for proto2
            // (because in proto3 first enum variant number is always 0).
            // `Default` implemented unconditionally to simplify certain
            // generic operations, e. g. reading a map.
            // Also, note that even in proto2 some operations fallback to
            // first enum value, e. g. `get_xxx` for unset field,
            // so this implementation is not completely unreasonable.
            w.comment("Note, `Default` is implemented although default value is not 0");
        }
        w.impl_for_block("::std::default::Default", &self.type_name, |w| {
            w.def_fn("default() -> Self", |w| {
                w.write_line(&format!(
                    "{}::{}",
                    &self.type_name,
                    &first_value.rust_name()
                ))
            });
        });
    }
}
