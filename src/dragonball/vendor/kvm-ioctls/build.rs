// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
    // Define a `has_sev` attribute, which is used for conditional
    // execution of SEV-specific tests and examples.
    if std::path::Path::new("/dev/sev").exists() {
        println!("cargo:rustc-cfg=has_sev");
    }
}
