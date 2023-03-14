// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use std::env;

const LIBSECCOMP_LIB_PATH: &str = "LIBSECCOMP_LIB_PATH";
const LIBSECCOMP_LINK_TYPE: &str = "LIBSECCOMP_LINK_TYPE";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed={}", LIBSECCOMP_LIB_PATH);
    println!("cargo:rerun-if-env-changed={}", LIBSECCOMP_LINK_TYPE);

    if let Ok(path) = env::var(LIBSECCOMP_LIB_PATH) {
        println!("cargo:rustc-link-search=native={}", path);
    }

    let link_type = match env::var(LIBSECCOMP_LINK_TYPE) {
        Ok(link_type) if link_type == "framework" => {
            return Err("Seccomp is a Linux specific technology".into());
        }
        Ok(link_type) => link_type, // static or dylib
        Err(_) => String::from("dylib"),
    };

    println!("cargo:rustc-link-lib={}=seccomp", link_type);

    Ok(())
}
