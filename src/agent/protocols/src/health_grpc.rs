// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]

const METHOD_HEALTH_CHECK: ::grpcio::Method<super::health::CheckRequest, super::health::HealthCheckResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.Health/Check",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_HEALTH_VERSION: ::grpcio::Method<super::health::CheckRequest, super::health::VersionCheckResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.Health/Version",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

#[derive(Clone)]
pub struct HealthClient {
    client: ::grpcio::Client,
}

impl HealthClient {
    pub fn new(channel: ::grpcio::Channel) -> Self {
        HealthClient {
            client: ::grpcio::Client::new(channel),
        }
    }

    pub fn check_opt(&self, req: &super::health::CheckRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::health::HealthCheckResponse> {
        self.client.unary_call(&METHOD_HEALTH_CHECK, req, opt)
    }

    pub fn check(&self, req: &super::health::CheckRequest) -> ::grpcio::Result<super::health::HealthCheckResponse> {
        self.check_opt(req, ::grpcio::CallOption::default())
    }

    pub fn check_async_opt(&self, req: &super::health::CheckRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::health::HealthCheckResponse>> {
        self.client.unary_call_async(&METHOD_HEALTH_CHECK, req, opt)
    }

    pub fn check_async(&self, req: &super::health::CheckRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::health::HealthCheckResponse>> {
        self.check_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn version_opt(&self, req: &super::health::CheckRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::health::VersionCheckResponse> {
        self.client.unary_call(&METHOD_HEALTH_VERSION, req, opt)
    }

    pub fn version(&self, req: &super::health::CheckRequest) -> ::grpcio::Result<super::health::VersionCheckResponse> {
        self.version_opt(req, ::grpcio::CallOption::default())
    }

    pub fn version_async_opt(&self, req: &super::health::CheckRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::health::VersionCheckResponse>> {
        self.client.unary_call_async(&METHOD_HEALTH_VERSION, req, opt)
    }

    pub fn version_async(&self, req: &super::health::CheckRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::health::VersionCheckResponse>> {
        self.version_async_opt(req, ::grpcio::CallOption::default())
    }
    pub fn spawn<F>(&self, f: F) where F: ::futures::Future<Item = (), Error = ()> + Send + 'static {
        self.client.spawn(f)
    }
}

pub trait Health {
    fn check(&mut self, ctx: ::grpcio::RpcContext, req: super::health::CheckRequest, sink: ::grpcio::UnarySink<super::health::HealthCheckResponse>);
    fn version(&mut self, ctx: ::grpcio::RpcContext, req: super::health::CheckRequest, sink: ::grpcio::UnarySink<super::health::VersionCheckResponse>);
}

pub fn create_health<S: Health + Send + Clone + 'static>(s: S) -> ::grpcio::Service {
    let mut builder = ::grpcio::ServiceBuilder::new();
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_HEALTH_CHECK, move |ctx, req, resp| {
        instance.check(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_HEALTH_VERSION, move |ctx, req, resp| {
        instance.version(ctx, req, resp)
    });
    builder.build()
}
