// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! Host device provisioning for a rootless VMM.
//!
//! TEE devices use dedicated groups so access to `/dev/kvm` does not also grant
//! access to platform-wide TEE interfaces. `/dev/vhost-vsock` and
//! `/dev/vhost-net` are excluded because the shim opens them before dropping
//! privileges and passes their file descriptors to QEMU.

use anyhow::{anyhow, Context, Result};
use log::{info, warn};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::utils::system;

const HOST_DEV: &str = "/host-dev";

const HOST_UDEV_RULES_DIR: &str = "/host-udev-rules.d";

const REQUIRED_PERMISSIONS: u32 = 0o6;

// Another installer may create the group between lookup and groupadd.
const GROUPADD_NAME_IN_USE: i32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostDevice {
    name: &'static str,
    group: &'static str,
    needed_by: &'static str,
}

const KVM: HostDevice = HostDevice {
    name: "kvm",
    group: "kvm",
    needed_by: "every VMM",
};

const SEV: HostDevice = HostDevice {
    name: "sev",
    group: "kata-sev",
    needed_by: "AMD SEV-SNP guest launch",
};

const UV: HostDevice = HostDevice {
    name: "uv",
    group: "kata-se",
    needed_by: "IBM Secure Execution attestation",
};

pub fn shim_supports_rootless(shim: &str) -> bool {
    // runtime-rs shims validate host device access before dropping privileges
    // and are the only ones that support TEE devices. clh-runtime-rs does not
    // support TEE, but it does support rootless for non-confidential workloads.
    system::is_rust_shim(shim) && (shim.starts_with("qemu") || shim.starts_with("clh"))
}

/// The devices the rootless `shims` need. A node that only made plain QEMU
/// rootless never gets the TEE groups.
fn devices_for(shims: &[&str]) -> Result<Vec<HostDevice>> {
    // The chart rejects these, so reaching one means ROOTLESS was set by hand.
    if let Some(shim) = shims.iter().find(|shim| !shim_supports_rootless(shim)) {
        return Err(anyhow!(
            "ROOTLESS asks for a rootless VMM on {shim}, which does not run one. Only the \
             runtime-rs QEMU shims (qemu-runtime-rs and its variants) and clh-runtime-rs do."
        ));
    }

    if shims.is_empty() {
        return Ok(Vec::new());
    }

    let mut devices = vec![KVM];

    if shims.iter().any(|shim| shim.contains("snp")) {
        devices.push(SEV);
    }
    if shims.iter().any(|shim| shim.starts_with("qemu-se-")) {
        devices.push(UV);
    }

    Ok(devices)
}

pub fn provision_host_device_access(config: &Config) -> Result<()> {
    let shims: Vec<&str> = config
        .rootless_shims_for_arch
        .iter()
        .map(String::as_str)
        .collect();

    let devices = devices_for(&shims)?;
    if devices.is_empty() {
        info!("install (rootless): no shim asked for a rootless VMM, leaving host devices alone");
        return Ok(());
    }

    let mut provisioned = Vec::new();
    for device in &devices {
        match reconcile_device(device) {
            Ok(Reconciled::Absent) => info!(
                "install (rootless): /dev/{} is not present on this node, skipping it",
                device.name
            ),
            Ok(Reconciled::AlreadyAccessible) => {
                info!(
                    "install (rootless): /dev/{} already grants an unprivileged VMM access",
                    device.name
                );
                provisioned.push(*device);
            }
            Ok(Reconciled::Provisioned { gid }) => {
                info!(
                    "install (rootless): /dev/{} is now group {} ({}) with group read/write, for {}",
                    device.name, device.group, gid, device.needed_by
                );
                provisioned.push(*device);
            }
            Err(error) => warn!(
                "install (rootless): could not provision /dev/{}, which {} needs: {error:#}. A \
                 rootless sandbox using it will fail to start until the node grants an \
                 unprivileged VMM read/write access to it.",
                device.name, device.needed_by
            ),
        }
    }

    // Record already-accessible devices because their current permissions may
    // not survive recreation; omit failures because udev ignores unknown groups.
    if provisioned.is_empty() {
        info!("install (rootless): no host device on this node could be provisioned");
        return remove_udev_rule(config);
    }

    write_udev_rule(config, &provisioned)
}

#[derive(Debug, PartialEq, Eq)]
enum Reconciled {
    Absent,
    AlreadyAccessible,
    Provisioned { gid: u32 },
}

fn reconcile_device(device: &HostDevice) -> Result<Reconciled> {
    let path = PathBuf::from(HOST_DEV).join(device.name);

    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Reconciled::Absent)
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to stat {}", path.display()))
        }
    };

    if !is_char_device(metadata.mode()) {
        return Err(anyhow!(
            "{} is not a character device (mode {:04o}); the shim refuses to authorize it",
            path.display(),
            metadata.mode() & 0o7777
        ));
    }

    // The persistent rule needs this group even when the live node is usable.
    let gid = ensure_host_group(device.group)?;

    if grants_unprivileged_access(metadata.mode(), metadata.gid()) {
        return Ok(Reconciled::AlreadyAccessible);
    }

    // Do not widen "other", which would grant every host process access.
    let mode = (metadata.mode() & 0o7777) | (REQUIRED_PERMISSIONS << 3);
    std::os::unix::fs::chown(&path, None, Some(gid))
        .with_context(|| format!("failed to set the group of {} to {gid}", path.display()))?;
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(mode))
        .with_context(|| format!("failed to set the mode of {} to {mode:04o}", path.display()))?;

    Ok(Reconciled::Provisioned { gid })
}

fn grants_unprivileged_access(mode: u32, gid: u32) -> bool {
    if mode & REQUIRED_PERMISSIONS == REQUIRED_PERMISSIONS {
        return true;
    }

    let group_permissions = REQUIRED_PERMISSIONS << 3;
    gid != 0 && mode & group_permissions == group_permissions
}

fn is_char_device(mode: u32) -> bool {
    mode & libc::S_IFMT == libc::S_IFCHR
}

fn ensure_host_group(group: &str) -> Result<u32> {
    if let Some(gid) = host_group_gid(group)? {
        return reject_root_group(group, gid);
    }

    let groupadd = system::find_host_program_in_chroot(&[
        "/usr/sbin/groupadd",
        "/sbin/groupadd",
        "/usr/bin/groupadd",
        "/bin/groupadd",
    ])
    .with_context(|| {
        format!(
            "group {group} does not exist and no groupadd was found under {}; create the group on \
             the node, or install shadow-utils, before deploying a rootless VMM",
            system::HOST_ROOT
        )
    })?;

    // Use the host's allocation range and group database locking.
    let output = system::run_in_host_chroot(&groupadd, &["--system", group])?;
    let status = output.status.code();
    if !output.status.success() && status != Some(GROUPADD_NAME_IN_USE) {
        return Err(anyhow!(
            "host groupadd --system {group} failed (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let gid = host_group_gid(group)?.ok_or_else(|| {
        anyhow!(
            "host groupadd reported success for {group} but {}/etc/group has no such group; a \
             group database this install cannot read cannot be used for device access either",
            system::HOST_ROOT
        )
    })?;

    reject_root_group(group, gid)
}

fn reject_root_group(group: &str, gid: u32) -> Result<u32> {
    if gid == 0 {
        return Err(anyhow!(
            "group {group} is GID 0, which the shim refuses to add to an unprivileged VMM"
        ));
    }
    Ok(gid)
}

fn host_group_gid(group: &str) -> Result<Option<u32>> {
    let path = Path::new(system::HOST_ROOT).join("etc/group");
    let database = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read the host group database {}", path.display()))?;

    Ok(group_gid_in(&database, group))
}

fn group_gid_in(database: &str, group: &str) -> Option<u32> {
    database.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != group {
            return None;
        }
        fields.next()?;
        fields.next()?.trim().parse().ok()
    })
}

fn udev_rule_path(base: &Path, multi_install_suffix: Option<&str>) -> Result<PathBuf> {
    let suffix = multi_install_suffix.unwrap_or("default");
    anyhow::ensure!(
        suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
        "MULTI_INSTALL_SUFFIX {suffix:?} cannot be used in a udev rules filename"
    );
    Ok(base.join(format!("99-kata-containers-rootless-{suffix}.rules")))
}

fn udev_rule_content(devices: &[HostDevice]) -> String {
    let mut content = String::from(
        "# Generated by kata-deploy: device access for a rootless (unprivileged) VMM.\n\
         # Removed on uninstall. Do not edit; change the chart's rootless settings instead.\n",
    );

    for device in devices {
        content.push_str(&format!(
            "# {}\nKERNEL==\"{}\", SUBSYSTEM==\"misc\", GROUP=\"{}\", MODE=\"0660\"\n",
            device.needed_by, device.name, device.group
        ));
    }

    content
}

fn write_udev_rule(config: &Config, devices: &[HostDevice]) -> Result<()> {
    let path = udev_rule_path(
        Path::new(HOST_UDEV_RULES_DIR),
        config.multi_install_suffix.as_deref(),
    )?;
    let content = udev_rule_content(devices);

    // Avoid udev observing a partially written policy.
    let temp_path = path.with_extension(format!("rules.{}.tmp", std::process::id()));
    let write_result = (|| -> Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut temp = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o644)
            .open(&temp_path)
            .with_context(|| {
                format!(
                    "failed to create the temporary udev rules file {}",
                    temp_path.display()
                )
            })?;
        temp.write_all(content.as_bytes())
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        temp.sync_all()
            .with_context(|| format!("failed to sync {}", temp_path.display()))?;
        std::fs::rename(&temp_path, &path).with_context(|| {
            format!(
                "failed to atomically install the udev rules file {}",
                path.display()
            )
        })?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result?;

    info!(
        "install (rootless): recorded rootless device access in {}",
        path.display()
    );
    Ok(())
}

pub fn remove_udev_rule(config: &Config) -> Result<()> {
    let path = udev_rule_path(
        Path::new(HOST_UDEV_RULES_DIR),
        config.multi_install_suffix.as_deref(),
    )?;

    match std::fs::remove_file(&path) {
        Ok(()) => info!(
            "rootless: removed the rootless device access rule {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to remove udev rules file {}", path.display()))
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("qemu-runtime-rs", true)]
    #[case("qemu-snp-runtime-rs", true)]
    #[case("qemu-se-runtime-rs", true)]
    #[case("qemu-nvidia-gpu-snp-runtime-rs", true)]
    #[case("clh-runtime-rs", true)]
    #[case("qemu", false)]
    #[case("qemu-snp", false)]
    #[case("qemu-se", false)]
    #[case("dragonball", false)]
    #[case("remote", false)]
    fn runtime_rs_qemu_and_clh_run_rootless(#[case] shim: &str, #[case] expected: bool) {
        assert_eq!(shim_supports_rootless(shim), expected);
    }

    #[test]
    fn a_plain_rootless_shim_asks_for_no_tee_device() {
        assert_eq!(devices_for(&["qemu-runtime-rs"]).unwrap(), vec![KVM]);
        assert_eq!(devices_for(&["clh-runtime-rs"]).unwrap(), vec![KVM]);
    }

    #[test]
    fn no_shim_asking_for_rootless_asks_for_nothing() {
        assert!(devices_for(&[]).unwrap().is_empty());
    }

    #[test]
    fn each_tee_is_asked_for_by_its_own_shims() {
        assert_eq!(
            devices_for(&["qemu-snp-runtime-rs"]).unwrap(),
            vec![KVM, SEV]
        );
        assert_eq!(
            devices_for(&["qemu-nvidia-gpu-snp-runtime-rs"]).unwrap(),
            vec![KVM, SEV]
        );
        assert_eq!(devices_for(&["qemu-se-runtime-rs"]).unwrap(), vec![KVM, UV]);

        assert_eq!(devices_for(&["qemu-tdx-runtime-rs"]).unwrap(), vec![KVM]);
    }

    #[test]
    fn a_tee_shim_left_privileged_does_not_bring_in_its_device() {
        // The node runs qemu-snp-runtime-rs, but only plain qemu-runtime-rs was
        // asked to go rootless, so /dev/sev stays as the node had it.
        assert_eq!(devices_for(&["qemu-runtime-rs"]).unwrap(), vec![KVM]);
    }

    #[test]
    fn one_node_running_both_tees_gets_both_devices() {
        assert_eq!(
            devices_for(&["qemu-snp-runtime-rs", "qemu-se-runtime-rs"]).unwrap(),
            vec![KVM, SEV, UV]
        );
    }

    #[rstest]
    #[case("qemu")]
    #[case("qemu-snp")]
    #[case("dragonball")]
    #[case("openvmm-azure-runtime-rs")]
    fn a_shim_that_stays_privileged_is_refused(#[case] shim: &str) {
        let error = devices_for(&[shim]).unwrap_err().to_string();
        assert!(error.contains(shim), "{error}");
    }

    #[test]
    fn the_tee_devices_do_not_share_a_group_with_kvm() {
        assert_ne!(SEV.group, KVM.group);
        assert_ne!(UV.group, KVM.group);
        assert_ne!(SEV.group, UV.group);
    }

    #[test]
    fn a_device_the_shim_never_opens_by_path_is_not_provisioned() {
        for shims in [
            vec!["qemu-runtime-rs"],
            vec!["qemu-snp-runtime-rs"],
            vec!["qemu-se-runtime-rs"],
            vec!["qemu-tdx-runtime-rs"],
        ] {
            let devices = devices_for(&shims).unwrap();
            for excluded in ["vhost-vsock", "vhost-net"] {
                assert!(
                    !devices.iter().any(|device| device.name == excluded),
                    "{excluded} should not be provisioned for {shims:?}"
                );
            }
        }
    }

    #[rstest]
    #[case(0o600, 0, false)]
    #[case(0o660, 1000, true)]
    #[case(0o660, 0, false)]
    #[case(0o666, 0, true)]
    #[case(0o640, 1000, false)]
    #[case(0o644, 0, false)]
    fn unprivileged_access_matches_the_shim(
        #[case] mode: u32,
        #[case] gid: u32,
        #[case] expected: bool,
    ) {
        assert_eq!(grants_unprivileged_access(mode, gid), expected);
    }

    #[test]
    fn a_group_is_read_out_of_the_host_database() {
        let database = "root:x:0:\nkvm:x:104:\nkata-sev:x:988:\nkata-se:x:989:someone\n";

        assert_eq!(group_gid_in(database, "kvm"), Some(104));
        assert_eq!(group_gid_in(database, "kata-sev"), Some(988));
        assert_eq!(group_gid_in(database, "kata-se"), Some(989));
        assert_eq!(group_gid_in(database, "root"), Some(0));
        assert_eq!(group_gid_in(database, "kata"), None);
    }

    #[test]
    fn a_malformed_group_line_is_not_mistaken_for_a_gid() {
        assert_eq!(group_gid_in("kata-sev\n", "kata-sev"), None);
        assert_eq!(group_gid_in("kata-sev:x:\n", "kata-sev"), None);
        assert_eq!(group_gid_in("kata-sev:x:nine:\n", "kata-sev"), None);
        assert_eq!(group_gid_in("", "kata-sev"), None);

        assert_eq!(
            group_gid_in("kata-sev:x:\nkata-sev:x:988:\n", "kata-sev"),
            Some(988)
        );
    }

    #[test]
    fn the_rule_file_sorts_after_the_distribution_rules() {
        let path = udev_rule_path(Path::new("/host-udev-rules.d"), None).unwrap();
        assert_eq!(
            path,
            Path::new("/host-udev-rules.d/99-kata-containers-rootless-default.rules")
        );

        let suffixed = udev_rule_path(Path::new("/host-udev-rules.d"), Some("dev")).unwrap();
        assert_eq!(
            suffixed,
            Path::new("/host-udev-rules.d/99-kata-containers-rootless-dev.rules")
        );
    }

    #[test]
    fn a_suffix_cannot_escape_the_rules_directory() {
        assert!(udev_rule_path(Path::new("/host-udev-rules.d"), Some("../../etc")).is_err());
        assert!(udev_rule_path(Path::new("/host-udev-rules.d"), Some("a/b")).is_err());
    }

    #[test]
    fn the_rule_names_each_device_and_its_group() {
        let content = udev_rule_content(&[KVM, SEV, UV]);

        assert!(content.contains(r#"KERNEL=="kvm", SUBSYSTEM=="misc", GROUP="kvm", MODE="0660""#));
        assert!(
            content.contains(r#"KERNEL=="sev", SUBSYSTEM=="misc", GROUP="kata-sev", MODE="0660""#)
        );
        assert!(
            content.contains(r#"KERNEL=="uv", SUBSYSTEM=="misc", GROUP="kata-se", MODE="0660""#)
        );
        assert!(!content.contains("vhost-vsock"));
    }
}
