// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(clippy::module_inception)]

#[cfg(test)]
mod test_utils {
    #[macro_export]
    #[allow(unused_macros)]
    macro_rules! skip_if_root {
        () => {
            if nix::unistd::Uid::effective().is_root() {
                println!("INFO: skipping {} which needs non-root", module_path!());
                return;
            }
        };
    }

    #[macro_export]
    #[allow(unused_macros)]
    macro_rules! skip_if_not_root {
        () => {
            if !nix::unistd::Uid::effective().is_root() {
                println!("INFO: skipping {} which needs root", module_path!());
                return;
            }
        };
    }

    #[macro_export]
    #[allow(unused_macros)]
    macro_rules! skip_loop_if_root {
        ($msg:expr) => {
            if nix::unistd::Uid::effective().is_root() {
                println!(
                    "INFO: skipping loop {} in {} which needs non-root",
                    $msg,
                    module_path!()
                );
                continue;
            }
        };
    }

    #[macro_export]
    #[allow(unused_macros)]
    macro_rules! skip_loop_if_not_root {
        ($msg:expr) => {
            if !nix::unistd::Uid::effective().is_root() {
                println!(
                    "INFO: skipping loop {} in {} which needs root",
                    $msg,
                    module_path!()
                );
                continue;
            }
        };
    }
}
