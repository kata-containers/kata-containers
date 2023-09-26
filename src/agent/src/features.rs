// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Returns a sorted list of optional features enabled at agent build time.
pub fn get_build_features() -> Vec<String> {
    let features: Vec<&str> = vec![
        #[cfg(feature = "agent-policy")]
        "agent-policy",
        #[cfg(feature = "guest-pull")]
        "guest-pull",
        #[cfg(feature = "seccomp")]
        "seccomp",
        #[cfg(feature = "standard-oci-runtime")]
        "standard-oci-runtime",
    ];

    let mut sorted: Vec<String> = features.into_iter().map(String::from).collect();

    sorted.sort();

    sorted
}
