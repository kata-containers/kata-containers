// Copyright (C) 2024 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

pub fn sl() -> slog::Logger {
    slog_scope::logger().new(slog::o!("subsystem" => "mem-agent"))
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        slog::info!(crate::misc::sl(), "{}", format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        slog::info!(crate::misc::sl(), "{}", format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        slog::info!(crate::misc::sl(), "{}", format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        slog::info!(crate::misc::sl(), "{}", format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        slog::info!(crate::misc::sl(), "{}", format_args!($($arg)*))
    }
}

#[cfg(test)]
pub fn is_test_environment() -> bool {
    true
}

#[cfg(not(test))]
pub fn is_test_environment() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_check() {
        assert!(is_test_environment());
    }

    #[test]
    fn test_log_macro() {
        error!("error");
        warn!("warn");
        info!("info");
        trace!("trace");
        debug!("debug");
    }
}
