// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use clap::Parser;

const DEFAULT_LISTEN_ADDRESS: &str = "127.0.0.1:8090";

pub const SANDBOX_PATH_RUNTIME_RS: &str = "/run/kata";

pub const SHIM_MONITOR_SOCK_NAME: &str = "shim-monitor.sock";

#[derive(Parser, Debug)]
#[command(name = "kata-monitor", about = "Kata Monitoring Daemon")]
pub struct CliArgs {
    #[arg(long, default_value = DEFAULT_LISTEN_ADDRESS)]
    pub listen_address: String,

    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub sandbox_path: &'static str,
}

impl RuntimeConfig {
    pub fn new() -> Self {
        Self {
            sandbox_path: SANDBOX_PATH_RUNTIME_RS,
        }
    }

    pub fn socket_path(&self, sandbox_id: &str) -> PathBuf {
        PathBuf::from(self.sandbox_path)
            .join(sandbox_id)
            .join(SHIM_MONITOR_SOCK_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path() {
        let config = RuntimeConfig::new();
        assert_eq!(
            config.socket_path("testpath"),
            PathBuf::from("/run/kata/testpath/shim-monitor.sock")
        );
    }

    #[test]
    fn test_sandbox_path() {
        let config = RuntimeConfig::new();
        assert_eq!(config.sandbox_path, SANDBOX_PATH_RUNTIME_RS);
    }
}
