// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Returns a sorted list of optional features enabled at agent build time.
pub fn get_build_features() -> Vec<String> {
    let features: Vec<&str> = vec![
        #[cfg(feature = "agent-policy")]
        "agent-policy",
        #[cfg(feature = "seccomp")]
        "seccomp",
        // Advertise the strict confidential-runtime policy behaviour so a shim or
        // verifier can distinguish a strict (closed-door, one-shot policy) guest from
        // a permissive one before relying on it.
        #[cfg(feature = "strict-policy")]
        "strict-policy",
    ];

    let mut sorted: Vec<String> = features.into_iter().map(String::from).collect();

    sorted.sort();

    sorted
}
