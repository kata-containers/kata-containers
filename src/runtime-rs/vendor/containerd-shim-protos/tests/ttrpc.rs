// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::Arc;

use containerd_shim_protos::api::{CreateTaskRequest, CreateTaskResponse, DeleteRequest};
use containerd_shim_protos::shim::shim_ttrpc::create_task;
use containerd_shim_protos::Task;
use protobuf::{CodedInputStream, CodedOutputStream, Message};
use ttrpc::{Code, MessageHeader, Request, Response, TtrpcContext};

const MESSAGE_TYPE_REQUEST: u8 = 0x1;
const MESSAGE_TYPE_RESPONSE: u8 = 0x2;

#[derive(Debug, PartialEq)]
struct FakeServer {
    magic: u32,
}

impl FakeServer {
    fn new() -> Self {
        FakeServer { magic: 0xadcbdacf }
    }
}

impl Task for FakeServer {
    fn create(
        &self,
        _ctx: &::ttrpc::TtrpcContext,
        req: CreateTaskRequest,
    ) -> ::ttrpc::Result<CreateTaskResponse> {
        let mut resp = CreateTaskResponse::default();

        assert_eq!(&req.id, "test1");
        resp.set_pid(0x10c0);
        assert_eq!(resp.compute_size(), 3);

        Ok(resp)
    }
}

fn create_ttrpc_context() -> (
    TtrpcContext,
    std::sync::mpsc::Receiver<(MessageHeader, Vec<u8>)>,
) {
    let (res_tx, rx) = channel();
    let mh = MessageHeader {
        type_: MESSAGE_TYPE_REQUEST,
        ..Default::default()
    };
    let ctx = TtrpcContext {
        fd: -1,
        mh,
        res_tx,
        metadata: HashMap::new(),
        timeout_nano: 0,
    };

    (ctx, rx)
}

#[test]
fn test_task_method_num() {
    let server = Arc::new(Box::new(FakeServer::new()) as Box<dyn Task + Send + Sync>);
    let task = create_task(server.clone());

    assert_eq!(task.len(), 17);
}

#[test]
fn test_create_task() {
    let mut req = CreateTaskRequest::default();
    req.set_id("test1".to_owned());
    let mut buf = Vec::with_capacity(req.compute_size() as usize);
    let mut s = CodedOutputStream::vec(&mut buf);
    req.write_to(&mut s).unwrap();
    s.flush().unwrap();
    assert_eq!(buf.len(), 7);

    let (ctx, rx) = create_ttrpc_context();
    let mut request = Request::new();
    request.set_service("containerd.task.v2.Task".to_owned());
    request.set_method("Create".to_owned());
    request.set_payload(buf);
    request.set_timeout_nano(10000);
    request.set_metadata(ttrpc::context::to_pb(ctx.metadata.clone()));

    let server = Arc::new(Box::new(FakeServer::new()) as Box<dyn Task + Send + Sync>);
    let task = create_task(server.clone());
    let create = task.get("/containerd.task.v2.Task/Create").unwrap();
    create.handler(ctx, request).unwrap();

    let (header, msg) = rx.recv().unwrap();
    assert_eq!(header.length, 7);
    assert_eq!(header.type_, MESSAGE_TYPE_RESPONSE);
    assert_eq!(header.flags, 0);
    assert_eq!(msg.len(), 7);

    let mut s = CodedInputStream::from_bytes(&msg);
    let mut response = Response::new();
    response.merge_from(&mut s).unwrap();
    assert_eq!(response.status.as_ref().unwrap().code, Code::OK);

    let mut s = CodedInputStream::from_bytes(&response.payload);
    let mut resp = CreateTaskResponse::new();
    resp.merge_from(&mut s).unwrap();
    assert_eq!(resp.pid, 0x10c0);
}

#[test]
fn test_delete_task() {
    let mut req = DeleteRequest::default();
    req.set_id("test1".to_owned());
    let mut buf = Vec::with_capacity(req.compute_size() as usize);
    let mut s = CodedOutputStream::vec(&mut buf);
    req.write_to(&mut s).unwrap();
    s.flush().unwrap();
    assert_eq!(buf.len(), 7);

    let (ctx, rx) = create_ttrpc_context();
    let mut request = Request::new();
    request.set_service("containerd.task.v2.Task".to_owned());
    request.set_method("Delete".to_owned());
    request.set_payload(buf);
    request.set_timeout_nano(10000);
    request.set_metadata(ttrpc::context::to_pb(ctx.metadata.clone()));

    let server = Arc::new(Box::new(FakeServer::new()) as Box<dyn Task + Send + Sync>);
    let task = create_task(server.clone());
    let delete = task.get("/containerd.task.v2.Task/Delete").unwrap();
    delete.handler(ctx, request).unwrap();

    let (header, msg) = rx.recv().unwrap();
    assert_eq!(header.length, 54);
    assert_eq!(header.type_, MESSAGE_TYPE_RESPONSE);
    assert_eq!(header.flags, 0);
    assert_eq!(msg.len(), 54);

    let mut s = CodedInputStream::from_bytes(&msg);
    let mut response = Response::new();
    response.merge_from(&mut s).unwrap();
    assert_ne!(response.status.as_ref().unwrap().code, Code::OK);
}
