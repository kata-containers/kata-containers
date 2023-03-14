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

use containerd_shim_protos::{api::CreateTaskRequest, TaskClient};

use ttrpc::context::{self, Context};
use ttrpc::Client;

fn main() {
    let c = Client::connect("unix:///tmp/shim-proto-ttrpc-001").unwrap();
    let task = TaskClient::new(c);
    let now = std::time::Instant::now();

    let mut req = CreateTaskRequest::new();
    req.set_id("id1".to_owned());
    println!(
        "OS Thread {:?} - task.create() started: {:?}",
        std::thread::current().id(),
        now.elapsed(),
    );
    let resp = task.create(default_ctx(), &req).unwrap();
    assert_eq!(resp.pid, 0x10c0);
    println!(
        "OS Thread {:?} - task.create() -> {:?} ended: {:?}",
        std::thread::current().id(),
        resp,
        now.elapsed(),
    );
}

fn default_ctx() -> Context {
    let mut ctx = context::with_timeout(0);
    ctx.add("key-1".to_string(), "value-1-1".to_string());
    ctx.add("key-1".to_string(), "value-1-2".to_string());
    ctx.set("key-2".to_string(), vec!["value-2".to_string()]);

    ctx
}
