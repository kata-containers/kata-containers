/*
   Copyright The containerd Authors.

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

use std::env;

use containerd_shim_protos as client;

use client::api;
use ttrpc::context::Context;

fn main() {
    let args: Vec<String> = env::args().collect();

    let socket_path = args
        .get(1)
        .ok_or("First argument must be shim socket path")
        .unwrap();

    let pid = args.get(2).map(|str| str.to_owned()).unwrap_or_default();

    println!("Connecting to {}...", socket_path);
    let client = client::Client::connect(socket_path).expect("Failed to connect to shim");

    let task_client = client::TaskClient::new(client);

    let context = Context::default();

    let req = api::ConnectRequest {
        id: pid,
        ..Default::default()
    };

    println!("Sending `Connect` request...");
    let resp = task_client
        .connect(context.clone(), &req)
        .expect("Connect request failed");
    println!("Connect response: {:?}", resp);

    let req = api::ShutdownRequest {
        id: "123".to_string(),
        now: true,
        ..Default::default()
    };

    println!("Sending `Shutdown` request...");
    let resp = task_client
        .shutdown(context, &req)
        .expect("Failed to send shutdown request");

    println!("Shutdown response: {:?}", resp)
}
