// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::policy::KataLinuxNamespace;

// Subset of namespaces applicable to Kata Containers.
pub fn get_namespaces(
    is_pause_container: bool,
    use_host_network: bool,
) -> Vec<KataLinuxNamespace> {
    let mut namespaces: Vec<KataLinuxNamespace> = vec![KataLinuxNamespace {
        Type: "ipc".to_string(),
        Path: "".to_string(),
    }];

    if !is_pause_container || !use_host_network {
        namespaces.push(KataLinuxNamespace {
            Type: "uts".to_string(),
            Path: "".to_string(),
        });
    }

    namespaces.push(KataLinuxNamespace {
        Type: "mount".to_string(),
        Path: "".to_string(),
    });

    namespaces
}
