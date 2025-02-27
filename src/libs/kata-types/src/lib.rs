// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Constants and Data Types shared by Kata Containers components.

#![deny(missing_docs)]
#[macro_use]
extern crate slog;
#[macro_use]
extern crate serde;

/// Constants and data types related to annotations.
pub mod annotations;

/// Kata configuration information from configuration file.
pub mod config;

/// Constants and data types related to container.
pub mod container;

/// Constants and data types related to CPU.
pub mod cpu;

/// Contants and data types related to device.
pub mod device;

/// Constants and data types related to handler.
pub mod handler;

/// Constants and data types related to Kubernetes/kubelet.
pub mod k8s;

/// Constants and data types related to mount point.
pub mod mount;

pub(crate) mod utils;

/// hypervisor capabilities
pub mod capabilities;

/// Common error codes.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Invalid configuration list.
    #[error("invalid list {0}")]
    InvalidList(String),
}

/// Convenience macro to obtain the scoped logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

/// Helper to create std::io::Error(std::io::ErrorKind::Other)
#[macro_export]
macro_rules! eother {
    () => (std::io::Error::new(std::io::ErrorKind::Other, ""));
    ($fmt:expr) => ({
        std::io::Error::new(std::io::ErrorKind::Other, format!($fmt))
    });
    ($fmt:expr, $($arg:tt)*) => ({
        std::io::Error::new(std::io::ErrorKind::Other, format!($fmt, $($arg)*))
    });
}

/// Resolve a path to its final value.
#[macro_export]
macro_rules! resolve_path {
    ($field:expr, $fmt:expr) => {{
        if !$field.is_empty() {
            match Path::new(&$field).canonicalize() {
                Err(e) => Err(eother!($fmt, &$field, e)),
                Ok(path) => {
                    $field = path.to_string_lossy().to_string();
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }};
}

/// Validate a path.
#[macro_export]
macro_rules! validate_path {
    ($field:expr, $fmt:expr) => {{
        if !$field.is_empty() {
            Path::new(&$field)
                .canonicalize()
                .map_err(|e| eother!($fmt, &$field, e))
                .map(|_| ())
        } else {
            Ok(())
        }
    }};
}
