// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(not(feature = "generate"))]
fn main() {}

#[cfg(feature = "generate")]
fn main() {
    use ttrpc_codegen::Customize;

    let protos = vec![
        "proto/github.com/containerd/containerd/runtime/v2/task/shim.proto",
        "proto/github.com/containerd/containerd/api/types/mount.proto",
        "proto/github.com/containerd/containerd/api/types/task/task.proto",
        "proto/github.com/containerd/containerd/api/events_task.proto",
        "proto/github.com/containerd/cgroups/stats/v1/metrics.proto",
        "proto/google/protobuf/empty.proto",
    ];

    if let Err(e) = ttrpc_codegen::Codegen::new()
        .out_dir("src")
        .inputs(&protos)
        .include("proto")
        .rust_protobuf()
        .customize(Customize {
            ..Default::default()
        })
        .run()
    {
        panic!("Gen codes failed with error {:?}", e);
    }
}
