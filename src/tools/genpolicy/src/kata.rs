// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use oci::LinuxNamespace;

// Subset of namespaces applicable to Kata Containers.
pub fn get_namespaces(is_pause_container: bool, use_host_network: bool) -> Vec<LinuxNamespace> {
    let mut namespaces: Vec<LinuxNamespace> = Vec::new();

    namespaces.push(LinuxNamespace {
        r#type: "ipc".to_string(),
        path: "".to_string(),
    });

    if !is_pause_container || !use_host_network {
        namespaces.push(LinuxNamespace {
            r#type: "uts".to_string(),
            path: "".to_string(),
        });
    }

    namespaces.push(LinuxNamespace {
        r#type: "mount".to_string(),
        path: "".to_string(),
    });

    namespaces
}
