// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use ttrpc::client::Client;
use ttrpc::context::{self, Context};

fn main() {
    let c = Client::connect("unix:///tmp/shim-proto-ttrpc-001").unwrap();
    let task = shim_proto::shim_ttrpc::TaskClient::new(c);

    let tashc = task.clone();

    let now = std::time::Instant::now();

    let mut req = shim_proto::shim::CreateTaskRequest::new();
    req.set_id("id1".to_owned());
    println!(
        "OS Thread {:?} - {} started: {:?}",
        std::thread::current().id(),
        "task.create()",
        now.elapsed(),
    );
    let resp = tashc.create(default_ctx(), &req).unwrap();
    assert_eq!(resp.pid, 0x10c0);
    println!(
        "OS Thread {:?} - {} -> {:?} ended: {:?}",
        std::thread::current().id(),
        "task.create()",
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
