// Copyright (c) 2019 Ant Financial
//
// Copyright 2017 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

// Copyright (c) 2016, Stepan Koltsov
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::Customize;
use protobuf::compiler_plugin;
use protobuf::descriptor::*;
use protobuf::descriptorx::*;
use protobuf_codegen::code_writer::CodeWriter;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use super::util::{
    self, async_on, def_async_fn, fq_grpc, pub_async_fn, to_camel_case, to_snake_case, MethodType,
};

struct MethodGen<'a> {
    proto: &'a MethodDescriptorProto,
    package_name: String,
    service_name: String,
    service_path: String,
    root_scope: &'a RootScope<'a>,
    customize: &'a Customize,
}

impl<'a> MethodGen<'a> {
    fn new(
        proto: &'a MethodDescriptorProto,
        package_name: String,
        service_name: String,
        service_path: String,
        root_scope: &'a RootScope<'a>,
        customize: &'a Customize,
    ) -> MethodGen<'a> {
        MethodGen {
            proto,
            package_name,
            service_name,
            service_path,
            root_scope,
            customize,
        }
    }

    fn input(&self) -> String {
        format!(
            "super::{}",
            self.root_scope
                .find_message(self.proto.get_input_type())
                .rust_fq_name()
        )
    }

    fn output(&self) -> String {
        format!(
            "super::{}",
            self.root_scope
                .find_message(self.proto.get_output_type())
                .rust_fq_name()
        )
    }

    fn method_type(&self) -> (MethodType, String) {
        match (
            self.proto.get_client_streaming(),
            self.proto.get_server_streaming(),
        ) {
            (false, false) => (MethodType::Unary, fq_grpc("MethodType::Unary")),
            (true, false) => (
                MethodType::ClientStreaming,
                fq_grpc("MethodType::ClientStreaming"),
            ),
            (false, true) => (
                MethodType::ServerStreaming,
                fq_grpc("MethodType::ServerStreaming"),
            ),
            (true, true) => (MethodType::Duplex, fq_grpc("MethodType::Duplex")),
        }
    }

    fn service_name(&self) -> String {
        to_snake_case(&self.service_name)
    }

    fn name(&self) -> String {
        to_snake_case(self.proto.get_name())
    }

    fn struct_name(&self) -> String {
        to_camel_case(self.proto.get_name())
    }

    fn fq_name(&self) -> String {
        format!("\"{}/{}\"", self.service_path, &self.proto.get_name())
    }

    fn const_method_name(&self) -> String {
        format!(
            "METHOD_{}_{}",
            self.service_name().to_uppercase(),
            self.name().to_uppercase()
        )
    }

    fn write_definition(&self, w: &mut CodeWriter) {
        let head = format!(
            "const {}: {}<{}, {}> = {} {{",
            self.const_method_name(),
            fq_grpc("Method"),
            self.input(),
            self.output(),
            fq_grpc("Method")
        );
        let pb_mar = format!(
            "{} {{ ser: {}, de: {} }}",
            fq_grpc("Marshaller"),
            fq_grpc("pb_ser"),
            fq_grpc("pb_de")
        );
        w.block(&head, "};", |w| {
            w.field_entry("ty", &self.method_type().1);
            w.field_entry("name", &self.fq_name());
            w.field_entry("req_mar", &pb_mar);
            w.field_entry("resp_mar", &pb_mar);
        });
    }

    fn write_handler(&self, w: &mut CodeWriter) {
        w.block(
            &format!("struct {}Method {{", self.struct_name()),
            "}",
            |w| {
                w.write_line(&format!(
                    "service: Arc<std::boxed::Box<dyn {} + Send + Sync>>,",
                    self.service_name
                ));
            },
        );
        w.write_line("");
        if async_on(self.customize, "server") {
            self.write_handler_impl_async(w)
        } else {
            self.write_handler_impl(w)
        }
    }

    fn write_handler_impl(&self, w: &mut CodeWriter) {
        w.block(&format!("impl ::ttrpc::MethodHandler for {}Method {{", self.struct_name()), "}",
        |w| {
            w.block("fn handler(&self, ctx: ::ttrpc::TtrpcContext, req: ::ttrpc::Request) -> ::ttrpc::Result<()> {", "}",
            |w| {
                w.write_line(&format!("::ttrpc::request_handler!(self, ctx, req, {}, {}, {});",
                                        proto_path_to_rust_mod(self.root_scope.find_message(self.proto.get_input_type()).get_scope().get_file_descriptor().get_name()),
                                        self.root_scope.find_message(self.proto.get_input_type()).rust_name(),
                                        self.name()));
                w.write_line("Ok(())");
            });
        });
    }

    fn write_handler_impl_async(&self, w: &mut CodeWriter) {
        w.write_line("#[async_trait]");
        w.block(&format!("impl ::ttrpc::r#async::MethodHandler for {}Method {{", self.struct_name()), "}",
        |w| {
            w.block("async fn handler(&self, ctx: ::ttrpc::r#async::TtrpcContext, req: ::ttrpc::Request) -> ::ttrpc::Result<(u32, Vec<u8>)> {", "}",
            |w| {
                w.write_line(&format!("::ttrpc::async_request_handler!(self, ctx, req, {}, {}, {});",
                                        proto_path_to_rust_mod(self.root_scope.find_message(self.proto.get_input_type()).get_scope().get_file_descriptor().get_name()),
                                        self.root_scope.find_message(self.proto.get_input_type()).rust_name(),
                                        self.name()));
            });
        });
    }

    // Method signatures
    fn unary(&self, method_name: &str) -> String {
        format!(
            "{}(&self, req: &{}, timeout_nano: i64) -> {}<{}>",
            method_name,
            self.input(),
            fq_grpc("Result"),
            self.output()
        )
    }

    fn unary_async(&self, method_name: &str) -> String {
        format!(
            "{}(&mut self, req: &{}, timeout_nano: i64) -> {}<{}>",
            method_name,
            self.input(),
            fq_grpc("Result"),
            self.output()
        )
    }

    fn client_streaming(&self, method_name: &str) -> String {
        format!(
            "{}(&self) -> {}<({}<{}>, {}<{}>)>",
            method_name,
            fq_grpc("Result"),
            fq_grpc("ClientCStreamSender"),
            self.input(),
            fq_grpc("ClientCStreamReceiver"),
            self.output()
        )
    }

    fn client_streaming_opt(&self, method_name: &str) -> String {
        format!(
            "{}_opt(&self, opt: {}) -> {}<({}<{}>, {}<{}>)>",
            method_name,
            fq_grpc("CallOption"),
            fq_grpc("Result"),
            fq_grpc("ClientCStreamSender"),
            self.input(),
            fq_grpc("ClientCStreamReceiver"),
            self.output()
        )
    }

    fn server_streaming(&self, method_name: &str) -> String {
        format!(
            "{}(&self, req: &{}) -> {}<{}<{}>>",
            method_name,
            self.input(),
            fq_grpc("Result"),
            fq_grpc("ClientSStreamReceiver"),
            self.output()
        )
    }

    fn server_streaming_opt(&self, method_name: &str) -> String {
        format!(
            "{}_opt(&self, req: &{}, opt: {}) -> {}<{}<{}>>",
            method_name,
            self.input(),
            fq_grpc("CallOption"),
            fq_grpc("Result"),
            fq_grpc("ClientSStreamReceiver"),
            self.output()
        )
    }

    fn duplex_streaming(&self, method_name: &str) -> String {
        format!(
            "{}(&self) -> {}<({}<{}>, {}<{}>)>",
            method_name,
            fq_grpc("Result"),
            fq_grpc("ClientDuplexSender"),
            self.input(),
            fq_grpc("ClientDuplexReceiver"),
            self.output()
        )
    }

    fn duplex_streaming_opt(&self, method_name: &str) -> String {
        format!(
            "{}_opt(&self, opt: {}) -> {}<({}<{}>, {}<{}>)>",
            method_name,
            fq_grpc("CallOption"),
            fq_grpc("Result"),
            fq_grpc("ClientDuplexSender"),
            self.input(),
            fq_grpc("ClientDuplexReceiver"),
            self.output()
        )
    }

    fn write_client(&self, w: &mut CodeWriter) {
        let method_name = self.name();
        match self.method_type().0 {
            // Unary
            MethodType::Unary => {
                w.pub_fn(&self.unary(&method_name), |w| {
                    w.write_line(&format!(
                        "let mut cres = {}::new();",
                        self.output()
                    ));
                    w.write_line(&format!(
                        "::ttrpc::client_request!(self, req, timeout_nano, \"{}.{}\", \"{}\", cres);",
                        self.package_name,
                        self.service_name,
                        &self.proto.get_name(),
                    ));
                    w.write_line("Ok(cres)");
                });
            }

            _ => {}
        };
    }

    fn write_async_client(&self, w: &mut CodeWriter) {
        let method_name = self.name();
        match self.method_type().0 {
            // Unary
            MethodType::Unary => {
                pub_async_fn(w, &self.unary_async(&method_name), |w| {
                    w.write_line(&format!("let mut cres = {}::new();", self.output()));
                    w.write_line(&format!(
                        "::ttrpc::async_client_request!(self, req, timeout_nano, \"{}.{}\", \"{}\", cres);",
                        self.package_name,
                        self.service_name,
                        &self.proto.get_name(),
                    ));
                });
            }

            _ => {}
        };
    }

    fn write_service(&self, w: &mut CodeWriter) {
        let req_stream_type = format!("{}<{}>", fq_grpc("RequestStream"), self.input());
        let (req, req_type, _resp_type) = match self.method_type().0 {
            MethodType::Unary => ("req", self.input(), "UnarySink"),
            MethodType::ClientStreaming => ("stream", req_stream_type, "ClientStreamingSink"),
            MethodType::ServerStreaming => ("req", self.input(), "ServerStreamingSink"),
            MethodType::Duplex => ("stream", req_stream_type, "DuplexSink"),
        };

        let get_sig = |context_name| {
            format!(
                "{}(&self, _ctx: &{}, _{}: {}) -> ::ttrpc::Result<{}>",
                self.name(),
                fq_grpc(context_name),
                req,
                req_type,
                self.output()
            )
        };

        let cb = |w: &mut CodeWriter| {
            w.write_line(format!("Err(::ttrpc::Error::RpcStatus(::ttrpc::get_status(::ttrpc::Code::NOT_FOUND, \"/{}.{}/{} is not supported\".to_string())))",
            self.package_name,
            self.service_name, self.proto.get_name(),));
        };

        if async_on(self.customize, "server") {
            let sig = get_sig("r#async::TtrpcContext");
            def_async_fn(w, &sig, cb);
        } else {
            let sig = get_sig("TtrpcContext");
            w.def_fn(&sig, cb);
        }
    }

    fn write_bind(&self, w: &mut CodeWriter) {
        let mut method_handler_name = "::ttrpc::MethodHandler";

        if async_on(self.customize, "server") {
            method_handler_name = "::ttrpc::r#async::MethodHandler";
        }

        let s = format!("methods.insert(\"/{}.{}/{}\".to_string(),
                    std::boxed::Box::new({}Method{{service: service.clone()}}) as std::boxed::Box<dyn {} + Send + Sync>);",
                    self.package_name,
                    self.service_name,
                    self.proto.get_name(),
                    self.struct_name(),
                    method_handler_name,
                );
        w.write_line(&s);
    }
}

struct ServiceGen<'a> {
    proto: &'a ServiceDescriptorProto,
    methods: Vec<MethodGen<'a>>,
    customize: &'a Customize,
}

impl<'a> ServiceGen<'a> {
    fn new(
        proto: &'a ServiceDescriptorProto,
        file: &FileDescriptorProto,
        root_scope: &'a RootScope,
        customize: &'a Customize,
    ) -> ServiceGen<'a> {
        let service_path = if file.get_package().is_empty() {
            format!("{}", proto.get_name())
        } else {
            format!("{}.{}", file.get_package(), proto.get_name())
        };
        let methods = proto
            .get_method()
            .iter()
            .map(|m| {
                MethodGen::new(
                    m,
                    file.get_package().to_string(),
                    util::to_camel_case(proto.get_name()),
                    service_path.clone(),
                    root_scope,
                    &customize,
                )
            })
            .collect();

        ServiceGen {
            proto,
            methods,
            customize,
        }
    }

    fn service_name(&self) -> String {
        util::to_camel_case(self.proto.get_name())
    }

    fn client_name(&self) -> String {
        format!("{}Client", self.service_name())
    }

    fn write_client(&self, w: &mut CodeWriter) {
        if async_on(self.customize, "client") {
            self.write_async_client(w)
        } else {
            self.write_sync_client(w)
        }
    }

    fn write_sync_client(&self, w: &mut CodeWriter) {
        w.write_line("#[derive(Clone)]");
        w.pub_struct(&self.client_name(), |w| {
            w.field_decl("client", "::ttrpc::Client");
        });

        w.write_line("");

        w.impl_self_block(&self.client_name(), |w| {
            w.pub_fn("new(client: ::ttrpc::Client) -> Self", |w| {
                w.expr_block(&self.client_name(), |w| {
                    w.field_entry("client", "client");
                });
            });

            for method in &self.methods {
                w.write_line("");
                method.write_client(w);
            }
        });
    }

    fn write_async_client(&self, w: &mut CodeWriter) {
        w.write_line("#[derive(Clone)]");
        w.pub_struct(&self.client_name(), |w| {
            w.field_decl("client", "::ttrpc::r#async::Client");
        });

        w.write_line("");

        w.impl_self_block(&self.client_name(), |w| {
            w.pub_fn("new(client: ::ttrpc::r#async::Client) -> Self", |w| {
                w.expr_block(&self.client_name(), |w| {
                    w.field_entry("client", "client");
                });
            });

            for method in &self.methods {
                w.write_line("");
                method.write_async_client(w);
            }
        });
    }

    fn write_server(&self, w: &mut CodeWriter) {
        let mut trait_name = self.service_name();
        let mut method_handler_name = "::ttrpc::MethodHandler";
        if async_on(self.customize, "server") {
            w.write_line("#[async_trait]");
            trait_name = format!("{}: Sync", &self.service_name());
            method_handler_name = "::ttrpc::r#async::MethodHandler";
        }

        w.pub_trait(&trait_name.to_owned(), |w| {
            for method in &self.methods {
                method.write_service(w);
            }
        });

        w.write_line("");

        let s = format!(
            "create_{}(service: Arc<std::boxed::Box<dyn {} + Send + Sync>>) -> HashMap <String, Box<dyn {} + Send + Sync>>",
            to_snake_case(&self.service_name()),
            self.service_name(),
            method_handler_name,
        );

        w.pub_fn(&s, |w| {
            w.write_line("let mut methods = HashMap::new();");
            for method in &self.methods[0..self.methods.len()] {
                w.write_line("");
                method.write_bind(w);
            }
            w.write_line("");
            w.write_line("methods");
        });
    }

    fn write_method_definitions(&self, w: &mut CodeWriter) {
        for (i, method) in self.methods.iter().enumerate() {
            if i != 0 {
                w.write_line("");
            }

            method.write_definition(w);
        }
    }

    fn write_method_handlers(&self, w: &mut CodeWriter) {
        for (i, method) in self.methods.iter().enumerate() {
            if i != 0 {
                w.write_line("");
            }

            method.write_handler(w);
        }
    }

    fn write(&self, w: &mut CodeWriter) {
        self.write_client(w);
        w.write_line("");
        self.write_method_handlers(w);
        w.write_line("");
        self.write_server(w);
    }
}

pub fn write_generated_by(w: &mut CodeWriter, pkg: &str, version: &str) {
    w.write_line(format!(
        "// This file is generated by {pkg} {version}. Do not edit",
        pkg = pkg,
        version = version
    ));
    write_generated_common(w);
}

fn write_generated_common(w: &mut CodeWriter) {
    // https://secure.phabricator.com/T784
    w.write_line("// @generated");

    w.write_line("");
    w.comment("https://github.com/Manishearth/rust-clippy/issues/702");
    w.write_line("#![allow(unknown_lints)]");
    w.write_line("#![allow(clipto_camel_casepy)]");
    w.write_line("");
    w.write_line("#![cfg_attr(rustfmt, rustfmt_skip)]");
    w.write_line("");
    w.write_line("#![allow(box_pointers)]");
    w.write_line("#![allow(dead_code)]");
    w.write_line("#![allow(missing_docs)]");
    w.write_line("#![allow(non_camel_case_types)]");
    w.write_line("#![allow(non_snake_case)]");
    w.write_line("#![allow(non_upper_case_globals)]");
    w.write_line("#![allow(trivial_casts)]");
    w.write_line("#![allow(unsafe_code)]");
    w.write_line("#![allow(unused_imports)]");
    w.write_line("#![allow(unused_results)]");
}

fn gen_file(
    file: &FileDescriptorProto,
    root_scope: &RootScope,
    customize: &Customize,
) -> Option<compiler_plugin::GenResult> {
    if file.get_service().is_empty() {
        return None;
    }

    let base = protobuf::descriptorx::proto_path_to_rust_mod(file.get_name());

    let mut v = Vec::new();
    {
        let mut w = CodeWriter::new(&mut v);

        write_generated_by(&mut w, "ttrpc-compiler", env!("CARGO_PKG_VERSION"));

        w.write_line("use protobuf::{CodedInputStream, CodedOutputStream, Message};");
        w.write_line("use std::collections::HashMap;");
        w.write_line("use std::sync::Arc;");
        if customize.async_all || customize.async_client || customize.async_server {
            w.write_line("use async_trait::async_trait;");
        }

        for service in file.get_service() {
            w.write_line("");
            ServiceGen::new(service, file, root_scope, customize).write(&mut w);
        }
    }

    Some(compiler_plugin::GenResult {
        name: base + "_ttrpc.rs",
        content: v,
    })
}

pub fn gen(
    file_descriptors: &[FileDescriptorProto],
    files_to_generate: &[String],
    customize: &Customize,
) -> Vec<compiler_plugin::GenResult> {
    let files_map: HashMap<&str, &FileDescriptorProto> =
        file_descriptors.iter().map(|f| (f.get_name(), f)).collect();

    let root_scope = RootScope { file_descriptors };

    let mut results = Vec::new();

    for file_name in files_to_generate {
        let file = files_map[&file_name[..]];

        if file.get_service().is_empty() {
            continue;
        }

        results.extend(gen_file(file, &root_scope, customize).into_iter());
    }

    results
}

pub fn gen_and_write(
    file_descriptors: &[FileDescriptorProto],
    files_to_generate: &[String],
    out_dir: &Path,
    customize: &Customize,
) -> io::Result<()> {
    let results = gen(file_descriptors, files_to_generate, customize);

    for r in &results {
        let mut file_path = out_dir.to_owned();
        file_path.push(&r.name);
        let mut file_writer = File::create(&file_path)?;
        file_writer.write_all(&r.content)?;
        file_writer.flush()?;
    }

    Ok(())
}

pub fn protoc_gen_grpc_rust_main() {
    compiler_plugin::plugin_main(|file_descriptors, files_to_generate| {
        gen(
            file_descriptors,
            files_to_generate,
            &Customize {
                ..Default::default()
            },
        )
    });
}
