// Copyright (c) 2019 Ant Financial
// Copyright (c) 2021 Ant Group
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;
use std::thread;

use containerd_shim_protos::{
    api::{CreateTaskRequest, CreateTaskResponse},
    create_task, Task,
};
use ttrpc::Server;

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
    simple_logger::SimpleLogger::new().init().unwrap();

    let t = Box::new(FakeServer::new()) as Box<dyn Task + Send + Sync>;
    let t = Arc::new(t);
    let tservice = create_task(t);

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
