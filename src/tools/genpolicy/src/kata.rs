// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use oci::LinuxNamespace;

// Subset of namespaces applicable to Kata Containers.
pub fn get_namespaces() -> Vec<LinuxNamespace> {
    vec![
        LinuxNamespace {
            r#type: "ipc".to_string(),
            path: "".to_string(),
        },
        LinuxNamespace {
            r#type: "uts".to_string(),
            path: "".to_string(),
        },
        LinuxNamespace {
            r#type: "mount".to_string(),
            path: "".to_string(),
        },
    ]
}
