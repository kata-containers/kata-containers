// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::time::{sleep, Instant};

// A sandbox cannot be stopped while it starts, so a device that is never
// released would block the shim forever without a limit.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(120);
const RELEASE_POLL_INTERVAL: Duration = Duration::from_secs(1);

const SYS_CLASS_VFIO_DEV: &str = "/sys/class/vfio-dev";

// The kernel keeps a PCI device enabled from the moment a user opens it
// until it is almost done releasing it, and shows that in sysfs. A VMM that
// binds the device after that is made to wait for the rest of the release,
// not refused, so the kernel does not have to be asked as well.
fn enabled(sysfs: &Path, cdev: &Path) -> bool {
    let Some(name) = cdev.file_name() else {
        return false;
    };
    fs::read_to_string(sysfs.join(name).join("device/enable"))
        .is_ok_and(|enable| enable.trim() != "0")
}

fn in_use(cdev: &Path) -> bool {
    enabled(Path::new(SYS_CLASS_VFIO_DEV), cdev)
}

/// Kubernetes can hand a device to a new pod while the VM of a pod that was
/// force-deleted still has it, and a VMM started then fails.
pub async fn wait_until_released(cdevs: Vec<PathBuf>) -> Result<()> {
    wait_while(cdevs, RELEASE_TIMEOUT, RELEASE_POLL_INTERVAL, in_use).await
}

async fn wait_while(
    cdevs: Vec<PathBuf>,
    timeout: Duration,
    interval: Duration,
    in_use: fn(&Path) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut waited = false;
    loop {
        let held: Vec<_> = cdevs
            .iter()
            .filter(|cdev| in_use(cdev))
            .map(|cdev| cdev.display().to_string())
            .collect();
        if held.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "{} still held by another process after {:?}",
                held.join(", "),
                timeout
            ));
        }
        if !waited {
            warn!(sl!(), "waiting for {} to be released", held.join(", "));
            waited = true;
        }
        sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const INTERVAL: Duration = Duration::from_millis(10);

    #[tokio::test]
    async fn a_device_is_waited_for_until_it_is_released() {
        static LOOKS: AtomicUsize = AtomicUsize::new(0);
        fn in_use_twice(_: &Path) -> bool {
            LOOKS.fetch_add(1, Ordering::SeqCst) < 2
        }
        let cdevs = vec![PathBuf::from("vfio0")];
        wait_while(cdevs, Duration::from_secs(5), INTERVAL, in_use_twice)
            .await
            .unwrap();
        assert_eq!(LOOKS.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_device_that_stays_held_is_named_in_the_error() {
        let cdevs = vec![PathBuf::from("vfio0"), PathBuf::from("vfio1")];
        let err = wait_while(cdevs, Duration::from_millis(50), INTERVAL, |cdev| {
            cdev.ends_with("vfio1")
        })
        .await
        .unwrap_err();
        assert!(err.to_string().starts_with("vfio1 still held"), "{}", err);
    }

    #[test]
    fn a_device_is_in_use_while_the_kernel_keeps_it_enabled() {
        let sysfs = tempfile::tempdir().unwrap();
        let device = sysfs.path().join("vfio0/device");
        fs::create_dir_all(&device).unwrap();
        let vfio0 = Path::new("/dev/vfio/devices/vfio0");

        assert!(!enabled(sysfs.path(), vfio0));
        fs::write(device.join("enable"), "1\n").unwrap();
        assert!(enabled(sysfs.path(), vfio0));
        fs::write(device.join("enable"), "0\n").unwrap();
        assert!(!enabled(sysfs.path(), vfio0));
    }
}
