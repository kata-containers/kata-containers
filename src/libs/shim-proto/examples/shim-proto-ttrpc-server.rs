// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;
use std::thread;

use log::LevelFilter;
use shim_proto::shim::{CreateTaskRequest, CreateTaskResponse};
use shim_proto::shim_ttrpc::Task;
use ttrpc::server::*;

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
        ctx: &::ttrpc::TtrpcContext,
        req: CreateTaskRequest,
    ) -> ::ttrpc::Result<CreateTaskResponse> {
        let mut resp = CreateTaskResponse::default();
        let md = &ctx.metadata;
        let v1 = md.get("key-1").unwrap();
        let v2 = md.get("key-2").unwrap();

        assert_eq!(v1[0], "value-1-1");
        assert_eq!(v1[1], "value-1-2");
        assert_eq!(v2[0], "value-2");
        assert_eq!(&req.id, "id1");

        resp.set_pid(0x10c0);

        Ok(resp)
    }
}

fn main() {
    simple_logging::log_to_stderr(LevelFilter::Trace);

    let t = Box::new(FakeServer::new()) as Box<dyn Task + Send + Sync>;
    let t = Arc::new(t);
    let tservice = shim_proto::shim_ttrpc::create_task(t);

    let mut server = Server::new()
        .bind("unix:///tmp/shim-proto-ttrpc-001")
        .unwrap()
        .register_service(tservice);

    server.start().unwrap();

    // Hold the main thread until receiving signal SIGTERM
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        ctrlc::set_handler(move || {
            tx.send(()).unwrap();
        })
        .expect("Error setting Ctrl-C handler");
        println!("Server is running, press Ctrl + C to exit");
    });

    rx.recv().unwrap();
}
