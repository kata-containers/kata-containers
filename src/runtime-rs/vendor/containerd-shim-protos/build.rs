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

#![allow(unused_imports)]

use std::fs;
use std::io::Write;
use std::path::Path;
#[cfg(feature = "generate_bindings")]
use ttrpc_codegen::{Codegen, Customize, ProtobufCustomize};

#[cfg(not(feature = "generate_bindings"))]
fn main() {}

#[cfg(feature = "generate_bindings")]
fn main() {
    codegen(
        "src/cgroups",
        &["vendor/github.com/containerd/cgroups/stats/v1/metrics.proto"],
        true,
        false,
    );

    codegen(
        "src/events",
        &[
            "vendor/github.com/containerd/containerd/api/events/container.proto",
            "vendor/github.com/containerd/containerd/api/events/content.proto",
            "vendor/github.com/containerd/containerd/api/events/image.proto",
            "vendor/github.com/containerd/containerd/api/events/namespace.proto",
            "vendor/github.com/containerd/containerd/api/events/snapshot.proto",
            "vendor/github.com/containerd/containerd/api/events/task.proto",
        ],
        false,
        false,
    );

    // generate async service
    codegen(
        "src/shim",
        &[
            "vendor/github.com/containerd/containerd/runtime/v2/task/shim.proto",
            "vendor/github.com/containerd/containerd/api/services/ttrpc/events/v1/events.proto",
        ],
        false,
        true,
    );
    fs::rename("src/shim/shim_ttrpc.rs", "src/shim/shim_ttrpc_async.rs").unwrap();
    fs::rename("src/shim/events_ttrpc.rs", "src/shim/events_ttrpc_async.rs").unwrap();

    codegen(
        "src/shim",
        &[
            "vendor/github.com/containerd/containerd/runtime/v2/runc/options/oci.proto",
            "vendor/github.com/containerd/containerd/runtime/v2/task/shim.proto",
            "vendor/github.com/containerd/containerd/api/services/ttrpc/events/v1/events.proto",
        ],
        false,
        false,
    );

    codegen(
        "src/types",
        &[
            "vendor/github.com/containerd/containerd/api/types/mount.proto",
            "vendor/github.com/containerd/containerd/api/types/task/task.proto",
            "vendor/google/protobuf/empty.proto",
        ],
        true,
        false,
    );
}

#[cfg(feature = "generate_bindings")]
fn codegen(
    path: impl AsRef<Path>,
    inputs: impl IntoIterator<Item = impl AsRef<Path>>,
    gen_mod_rs: bool,
    async_all: bool,
) {
    let path = path.as_ref();

    fs::create_dir_all(&path).unwrap();

    Codegen::new()
        .inputs(inputs)
        .include("vendor/")
        .rust_protobuf()
        .rust_protobuf_customize(ProtobufCustomize {
            gen_mod_rs: Some(gen_mod_rs),
            ..Default::default()
        })
        .customize(Customize {
            async_all,
            ..Default::default()
        })
        .out_dir(path)
        .run()
        .expect("Failed to generate protos");
}
