// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::pod;

use log::debug;

/// Adds a K8s pause container to a vector.
pub fn add_pause_container(containers: &mut Vec<pod::Container>) {
    debug!("Adding pause container...");
    let pause_container = pod::Container {
        image: "mcr.microsoft.com/oss/kubernetes/pause:3.6".to_string(),
        name: String::new(),
        imagePullPolicy: None,
        securityContext: Some(pod::SecurityContext {
            readOnlyRootFilesystem: Some(true),
            allowPrivilegeEscalation: Some(false),
            privileged: None,
        }),
        volumeMounts: None,
        env: None,
        resources: None,
        ports: None,
        command: None,
        args: None,
    };
    containers.insert(0, pause_container);
    debug!("pause container added.");
}
