// Copyright (c) 2019 Intel Corporation
// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[derive(Debug, PartialEq)]
pub enum TestUserType {
    RootOnly,
    NonRootOnly,
    Any,
}

#[macro_export]
macro_rules! skip_if_root {
    () => {
        if nix::unistd::Uid::effective().is_root() {
            println!("INFO: skipping {} which needs non-root", module_path!());
            return;
        }
    };
}

#[macro_export]
macro_rules! skip_if_not_root {
    () => {
        if !nix::unistd::Uid::effective().is_root() {
            println!("INFO: skipping {} which needs root", module_path!());
            return;
        }
    };
}

#[macro_export]
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

// Parameters:
//
// 1: expected Result
// 2: actual Result
// 3: string used to identify the test on error
#[macro_export]
macro_rules! assert_result {
    ($expected_result:expr, $actual_result:expr, $msg:expr) => {
        if $expected_result.is_ok() {
            let expected_value = $expected_result.as_ref().unwrap();
            let actual_value = $actual_result.unwrap();
            assert!(*expected_value == actual_value, "{}", $msg);
        } else {
            assert!($actual_result.is_err(), "{}", $msg);

            let expected_error = $expected_result.as_ref().unwrap_err();
            let expected_error_msg = format!("{:?}", expected_error);

            let actual_error_msg = format!("{:?}", $actual_result.unwrap_err());

            assert!(expected_error_msg == actual_error_msg, "{}", $msg);
        }
    };
}

#[macro_export]
macro_rules! skip_loop_by_user {
    ($msg:expr, $user:expr) => {
        if $user == TestUserType::RootOnly {
            skip_loop_if_not_root!($msg);
        } else if $user == TestUserType::NonRootOnly {
            skip_loop_if_root!($msg);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{skip_if_not_root, skip_if_root};

    #[test]
    fn test_skip_if_not_root() {
        skip_if_not_root!();
        assert!(
            nix::unistd::Uid::effective().is_root(),
            "normal user should be skipped"
        )
    }

    #[test]
    fn test_skip_if_root() {
        skip_if_root!();
        assert!(
            !nix::unistd::Uid::effective().is_root(),
            "root user should be skipped"
        )
    }
}
