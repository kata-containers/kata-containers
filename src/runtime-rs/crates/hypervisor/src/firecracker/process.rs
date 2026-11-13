// Copyright (c) 2026 Kata Contributors
// SPDX-License-Identifier: Apache-2.0
use super::FcInner;
use anyhow::{anyhow, bail, Context, Result};
use semver::Version;
use std::time::Duration;
use tokio::process::Command;
pub(super) const MIN_VERSION: &str = "0.21.1";
pub(super) fn parse_version(output: &str) -> Result<Version> {
    let mut fields = output.lines().next().unwrap_or_default().split_whitespace();
    if fields.next() != Some("Firecracker") {
        bail!("malformed Firecracker version: {output:?}");
    }
    let version = fields
        .next()
        .ok_or_else(|| anyhow!("missing Firecracker version"))?;
    let version = Version::parse(version.strip_prefix('v').unwrap_or(version))
        .context("malformed Firecracker version")?;
    if version < Version::parse(MIN_VERSION)? {
        bail!("Firecracker {version} is unsupported; minimum supported version is {MIN_VERSION}");
    }
    Ok(version)
}
impl FcInner {
    pub(super) async fn validate_version(&self) -> Result<()> {
        let output = tokio::time::timeout(
            Duration::from_secs(5),
            Command::new(&self.config.path)
                .arg("--version")
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Firecracker --version timed out")?
        .with_context(|| format!("read Firecracker version from {}", self.config.path))?;
        if !output.status.success() {
            bail!(
                "Firecracker --version failed: {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        parse_version(
            std::str::from_utf8(&output.stdout).context("non-UTF-8 Firecracker version")?,
        )?;
        Ok(())
    }
}
