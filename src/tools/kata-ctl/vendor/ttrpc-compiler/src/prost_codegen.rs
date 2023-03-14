// Copyright 2019 PingCAP, Inc.
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

use super::util::{fq_grpc, to_snake_case, MethodType};
use derive_new::new;
use prost::Message;
use prost_build::{protoc, protoc_include, Config, Method, Service, ServiceGenerator};
use prost_types::FileDescriptorSet;
use std::io::{Error, ErrorKind, Read};
use std::path::Path;
use std::{fs, io, process::Command};

/// Returns the names of all packages compiled.
pub fn compile_protos<P>(protos: &[P], includes: &[P], out_dir: &str) -> io::Result<Vec<String>>
where
    P: AsRef<Path>,
{
    let mut prost_config = Config::new();
    prost_config.service_generator(Box::new(Generator));
    prost_config.out_dir(out_dir);

    // Create a file descriptor set for the protocol files.
    let tmp = tempfile::Builder::new().prefix("prost-build").tempdir()?;
    let descriptor_set = tmp.path().join("prost-descriptor-set");

    let mut cmd = Command::new(protoc());
    cmd.arg("--include_imports")
        .arg("--include_source_info")
        .arg("-o")
        .arg(&descriptor_set);

    for include in includes {
        cmd.arg("-I").arg(include.as_ref());
    }

    // Set the protoc include after the user includes in case the user wants to
    // override one of the built-in .protos.
    cmd.arg("-I").arg(protoc_include());

    for proto in protos {
        cmd.arg(proto.as_ref());
    }

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(Error::new(
            ErrorKind::Other,
            format!("protoc failed: {}", String::from_utf8_lossy(&output.stderr)),
        ));
    }

    let mut buf = Vec::new();
    fs::File::open(descriptor_set)?.read_to_end(&mut buf)?;
    let descriptor_set = FileDescriptorSet::decode(&*buf)?;

    // Get the package names from the descriptor set.
    let mut packages: Vec<_> = descriptor_set
        .file
        .iter()
        .filter_map(|f| f.package.clone())
        .collect();
    packages.sort();
    packages.dedup();

    // FIXME(https://github.com/danburkert/prost/pull/155)
    // Unfortunately we have to forget the above work and use `compile_protos` to
    // actually generate the Rust code.
    prost_config.compile_protos(protos, includes)?;

    Ok(packages)
}

struct Generator;

impl ServiceGenerator for Generator {
    fn generate(&mut self, service: Service, buf: &mut String) {
        generate_methods(&service, buf);
        generate_client(&service, buf);
        generate_server(&service, buf);
    }
}

fn generate_methods(service: &Service, buf: &mut String) {
    let service_path = if service.package.is_empty() {
        format!("/{}", service.proto_name)
    } else {
        format!("/{}.{}", service.package, service.proto_name)
    };

    for method in &service.methods {
        generate_method(&service.name, &service_path, method, buf);
    }
}

fn const_method_name(service_name: &str, method: &Method) -> String {
    format!(
        "METHOD_{}_{}",
        to_snake_case(service_name).to_uppercase(),
        method.name.to_uppercase()
    )
}

fn generate_method(service_name: &str, service_path: &str, method: &Method, buf: &mut String) {
    let name = const_method_name(service_name, method);
    let ty = format!(
        "{}<{}, {}>",
        fq_grpc("Method"),
        method.input_type,
        method.output_type
    );

    buf.push_str("const ");
    buf.push_str(&name);
    buf.push_str(": ");
    buf.push_str(&ty);
    buf.push_str(" = ");
    generate_method_body(service_path, method, buf);
}

fn generate_method_body(service_path: &str, method: &Method, buf: &mut String) {
    let ty = fq_grpc(&MethodType::from_method(method).to_string());
    let pr_mar = format!(
        "{} {{ ser: {}, de: {} }}",
        fq_grpc("Marshaller"),
        fq_grpc("pr_ser"),
        fq_grpc("pr_de")
    );

    buf.push_str(&fq_grpc("Method"));
    buf.push('{');
    generate_field_init("ty", &ty, buf);
    generate_field_init(
        "name",
        &format!("\"{}/{}\"", service_path, method.proto_name),
        buf,
    );
    generate_field_init("req_mar", &pr_mar, buf);
    generate_field_init("resp_mar", &pr_mar, buf);
    buf.push_str("};\n");
}

// TODO share this code with protobuf codegen
impl MethodType {
    fn from_method(method: &Method) -> MethodType {
        match (method.client_streaming, method.server_streaming) {
            (false, false) => MethodType::Unary,
            (true, false) => MethodType::ClientStreaming,
            (false, true) => MethodType::ServerStreaming,
            (true, true) => MethodType::Duplex,
        }
    }
}

fn generate_field_init(name: &str, value: &str, buf: &mut String) {
    buf.push_str(name);
    buf.push_str(": ");
    buf.push_str(value);
    buf.push_str(", ");
}

fn generate_client(service: &Service, buf: &mut String) {
    let client_name = format!("{}Client", service.name);
    buf.push_str("#[derive(Clone)]\n");
    buf.push_str("pub struct ");
    buf.push_str(&client_name);
    buf.push_str(" { client: ::grpcio::Client }\n");

    buf.push_str("impl ");
    buf.push_str(&client_name);
    buf.push_str(" {\n");
    generate_ctor(&client_name, buf);
    generate_client_methods(service, buf);
    generate_spawn(buf);
    buf.push_str("}\n")
}

fn generate_ctor(client_name: &str, buf: &mut String) {
    buf.push_str("pub fn new(channel: ::grpcio::Channel) -> Self { ");
    buf.push_str(client_name);
    buf.push_str(" { client: ::grpcio::Client::new(channel) }");
    buf.push_str("}\n");
}

fn generate_client_methods(service: &Service, buf: &mut String) {
    for method in &service.methods {
        generate_client_method(&service.name, method, buf);
    }
}

fn generate_client_method(service_name: &str, method: &Method, buf: &mut String) {
    let name = &format!(
        "METHOD_{}_{}",
        to_snake_case(service_name).to_uppercase(),
        method.name.to_uppercase()
    );
    match MethodType::from_method(method) {
        MethodType::Unary => {
            ClientMethod::new(
                &method.name,
                true,
                Some(&method.input_type),
                false,
                vec![&method.output_type],
                "unary_call",
                name,
            )
            .generate(buf);
            ClientMethod::new(
                &method.name,
                false,
                Some(&method.input_type),
                false,
                vec![&method.output_type],
                "unary_call",
                name,
            )
            .generate(buf);
            ClientMethod::new(
                &method.name,
                true,
                Some(&method.input_type),
                true,
                vec![&format!(
                    "{}<{}>",
                    fq_grpc("ClientUnaryReceiver"),
                    method.output_type
                )],
                "unary_call",
                name,
            )
            .generate(buf);
            ClientMethod::new(
                &method.name,
                false,
                Some(&method.input_type),
                true,
                vec![&format!(
                    "{}<{}>",
                    fq_grpc("ClientUnaryReceiver"),
                    method.output_type
                )],
                "unary_call",
                name,
            )
            .generate(buf);
        }
        MethodType::ClientStreaming => {
            ClientMethod::new(
                &method.name,
                true,
                None,
                false,
                vec![
                    &format!("{}<{}>", fq_grpc("ClientCStreamSender"), method.input_type),
                    &format!(
                        "{}<{}>",
                        fq_grpc("ClientCStreamReceiver"),
                        method.output_type
                    ),
                ],
                "client_streaming",
                name,
            )
            .generate(buf);
            ClientMethod::new(
                &method.name,
                false,
                None,
                false,
                vec![
                    &format!("{}<{}>", fq_grpc("ClientCStreamSender"), method.input_type),
                    &format!(
                        "{}<{}>",
                        fq_grpc("ClientCStreamReceiver"),
                        method.output_type
                    ),
                ],
                "client_streaming",
                name,
            )
            .generate(buf);
        }
        MethodType::ServerStreaming => {
            ClientMethod::new(
                &method.name,
                true,
                Some(&method.input_type),
                false,
                vec![&format!(
                    "{}<{}>",
                    fq_grpc("ClientSStreamReceiver"),
                    method.output_type
                )],
                "server_streaming",
                name,
            )
            .generate(buf);
            ClientMethod::new(
                &method.name,
                false,
                Some(&method.input_type),
                false,
                vec![&format!(
                    "{}<{}>",
                    fq_grpc("ClientSStreamReceiver"),
                    method.output_type
                )],
                "server_streaming",
                name,
            )
            .generate(buf);
        }
        MethodType::Duplex => {
            ClientMethod::new(
                &method.name,
                true,
                None,
                false,
                vec![
                    &format!("{}<{}>", fq_grpc("ClientDuplexSender"), method.input_type),
                    &format!(
                        "{}<{}>",
                        fq_grpc("ClientDuplexReceiver"),
                        method.output_type
                    ),
                ],
                "duplex_streaming",
                name,
            )
            .generate(buf);
            ClientMethod::new(
                &method.name,
                false,
                None,
                false,
                vec![
                    &format!("{}<{}>", fq_grpc("ClientDuplexSender"), method.input_type),
                    &format!(
                        "{}<{}>",
                        fq_grpc("ClientDuplexReceiver"),
                        method.output_type
                    ),
                ],
                "duplex_streaming",
                name,
            )
            .generate(buf);
        }
    }
}

#[derive(new)]
struct ClientMethod<'a> {
    method_name: &'a str,
    opt: bool,
    request: Option<&'a str>,
    r#async: bool,
    result_types: Vec<&'a str>,
    inner_method_name: &'a str,
    data_name: &'a str,
}

impl<'a> ClientMethod<'a> {
    fn generate(&self, buf: &mut String) {
        buf.push_str("pub fn ");

        buf.push_str(self.method_name);
        if self.r#async {
            buf.push_str("_async");
        }
        if self.opt {
            buf.push_str("_opt");
        }

        buf.push_str("(&self");
        if let Some(req) = self.request {
            buf.push_str(", req: &");
            buf.push_str(req);
        }
        if self.opt {
            buf.push_str(", opt: ");
            buf.push_str(&fq_grpc("CallOption"));
        }
        buf.push_str(") -> ");

        buf.push_str(&fq_grpc("Result"));
        buf.push('<');
        if self.result_types.len() != 1 {
            buf.push('(');
        }
        for rt in &self.result_types {
            buf.push_str(rt);
            buf.push(',');
        }
        if self.result_types.len() != 1 {
            buf.push(')');
        }
        buf.push_str("> { ");
        if self.opt {
            self.generate_inner_body(buf);
        } else {
            self.generate_opt_body(buf);
        }
        buf.push_str(" }\n");
    }

    // Method delegates to the `_opt` version of the method.
    fn generate_opt_body(&self, buf: &mut String) {
        buf.push_str("self.");
        buf.push_str(self.method_name);
        if self.r#async {
            buf.push_str("_async");
        }
        buf.push_str("_opt(");
        if self.request.is_some() {
            buf.push_str("req, ");
        }
        buf.push_str(&fq_grpc("CallOption::default()"));
        buf.push(')');
    }

    // Method delegates to the inner client.
    fn generate_inner_body(&self, buf: &mut String) {
        buf.push_str("self.client.");
        buf.push_str(self.inner_method_name);
        if self.r#async {
            buf.push_str("_async");
        }
        buf.push_str("(&");
        buf.push_str(self.data_name);
        if self.request.is_some() {
            buf.push_str(", req");
        }
        buf.push_str(", opt)");
    }
}

fn generate_spawn(buf: &mut String) {
    buf.push_str(
        "pub fn spawn<F>(&self, f: F) \
         where F: ::futures::Future<Item = (), Error = ()> + Send + 'static {\
         self.client.spawn(f)\
         }\n",
    );
}

fn generate_server(service: &Service, buf: &mut String) {
    buf.push_str("pub trait ");
    buf.push_str(&service.name);
    buf.push_str(" {\n");
    generate_server_methods(service, buf);
    buf.push_str("}\n");

    buf.push_str("pub fn create_");
    buf.push_str(&to_snake_case(&service.name));
    buf.push_str("<S: ");
    buf.push_str(&service.name);
    buf.push_str(" + Send + Clone + 'static>(s: S) -> ");
    buf.push_str(&fq_grpc("Service"));
    buf.push_str(" {\n");
    buf.push_str("let mut builder = ::grpcio::ServiceBuilder::new();\n");

    for method in &service.methods[0..service.methods.len() - 1] {
        buf.push_str("let mut instance = s.clone();\n");
        generate_method_bind(&service.name, method, buf);
    }

    buf.push_str("let mut instance = s;\n");
    generate_method_bind(
        &service.name,
        &service.methods[service.methods.len() - 1],
        buf,
    );

    buf.push_str("builder.build()\n");
    buf.push_str("}\n");
}

fn generate_server_methods(service: &Service, buf: &mut String) {
    for method in &service.methods {
        let method_type = MethodType::from_method(method);
        let request_arg = match method_type {
            MethodType::Unary | MethodType::ServerStreaming => {
                format!("req: {}", method.input_type)
            }
            MethodType::ClientStreaming | MethodType::Duplex => format!(
                "stream: {}<{}>",
                fq_grpc("RequestStream"),
                method.input_type
            ),
        };
        let response_type = match method_type {
            MethodType::Unary => "UnarySink",
            MethodType::ClientStreaming => "ClientStreamingSink",
            MethodType::ServerStreaming => "ServerStreamingSink",
            MethodType::Duplex => "DuplexSink",
        };
        generate_server_method(method, &request_arg, response_type, buf);
    }
}

fn generate_server_method(
    method: &Method,
    request_arg: &str,
    response_type: &str,
    buf: &mut String,
) {
    buf.push_str("fn ");
    buf.push_str(&method.name);
    buf.push_str("(&mut self, ctx: ");
    buf.push_str(&fq_grpc("RpcContext"));
    buf.push_str(", ");
    buf.push_str(request_arg);
    buf.push_str(", sink: ");
    buf.push_str(&fq_grpc(response_type));
    buf.push('<');
    buf.push_str(&method.output_type);
    buf.push('>');
    buf.push_str(");\n");
}

fn generate_method_bind(service_name: &str, method: &Method, buf: &mut String) {
    let add_name = match MethodType::from_method(method) {
        MethodType::Unary => "add_unary_handler",
        MethodType::ClientStreaming => "add_client_streaming_handler",
        MethodType::ServerStreaming => "add_server_streaming_handler",
        MethodType::Duplex => "add_duplex_streaming_handler",
    };

    buf.push_str("builder = builder.");
    buf.push_str(add_name);
    buf.push_str("(&");
    buf.push_str(&const_method_name(service_name, method));
    buf.push_str(", move |ctx, req, resp| instance.");
    buf.push_str(&method.name);
    buf.push_str("(ctx, req, resp));\n");
}
