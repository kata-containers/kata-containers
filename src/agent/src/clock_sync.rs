// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

use slog::{info, warn, Logger};
use std::path::Path;
use tokio::process::Command;

const CHRONYD_PATH: &str = "/usr/sbin/chronyd";
const CHRONY_CONFIG_PATH: &str = "/etc/chrony/chrony.conf";
const KVM_PTP_DEVICE: &str = "/dev/ptp0";

fn available(chronyd: &Path, config: &Path, ptp_device: &Path) -> bool {
    chronyd.is_file() && config.is_file() && ptp_device.exists()
}

pub async fn start(logger: &Logger) {
    if !available(
        Path::new(CHRONYD_PATH),
        Path::new(CHRONY_CONFIG_PATH),
        Path::new(KVM_PTP_DEVICE),
    ) {
        warn!(logger, "guest clock synchronization unavailable");
        return;
    }

    info!(logger, "starting guest clock synchronization"; "source" => KVM_PTP_DEVICE);
    if let Err(err) = Command::new(CHRONYD_PATH).args(["-d", "-F", "2"]).spawn() {
        warn!(logger, "failed to start guest clock synchronization"; "error" => format!("{err:#}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_available() {
        let dir = tempdir().unwrap();
        let chronyd = dir.path().join("chronyd");
        let config = dir.path().join("chrony.conf");
        let ptp_device = dir.path().join("ptp0");

        fs::write(&chronyd, []).unwrap();
        fs::write(&config, []).unwrap();
        fs::write(&ptp_device, []).unwrap();

        assert!(available(&chronyd, &config, &ptp_device));

        fs::remove_file(&ptp_device).unwrap();
        assert!(!available(&chronyd, &config, &ptp_device));
    }
}
