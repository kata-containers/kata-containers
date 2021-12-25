// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

pub mod cgroup;
pub mod device;
pub mod fs;
pub mod hooks;
pub mod k8s;
pub mod mount;
pub mod numa;

// Convenience macro to obtain the scoped logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[macro_export]
macro_rules! eother {
    () => (std::io::Error::new(std::io::ErrorKind::Other, ""));
    ($fmt:expr, $($arg:tt)*) => ({
        std::io::Error::new(std::io::ErrorKind::Other, format!($fmt, $($arg)*))
    })
}
