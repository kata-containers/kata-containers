// Copyright 2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{bail, Context, Result};
use nix::unistd::gettid;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;

pub fn is_enabled() -> Result<bool> {
    let buf = fs::read_to_string("/proc/mounts")?;
    let enabled = buf.contains("selinuxfs");

    Ok(enabled)
}

pub fn add_mount_label(data: &mut String, label: &str) {
    if data.is_empty() {
        let context = format!("context=\"{label}\"");
        data.push_str(&context);
    } else {
        let context = format!(",context=\"{label}\"");
        data.push_str(&context);
    }
}

pub fn set_exec_label(label: &str) -> Result<()> {
    let mut attr_path = Path::new("/proc/thread-self/attr/exec").to_path_buf();
    if !attr_path.exists() {
        // Fall back to the old convention
        attr_path = Path::new("/proc/self/task")
            .join(gettid().to_string())
            .join("attr/exec")
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(attr_path)?;
    file.write_all(label.as_bytes())
        .with_context(|| "failed to apply SELinux label")?;

    Ok(())
}

/// Decide the SELinux context an exec'd process runs in.
///
/// The context recorded when the container was created is authoritative: a request with no
/// label inherits it, a request that repeats it is accepted, and any other request is
/// rejected rather than allowed to re-bind the process to a different identity.
///
/// A container registered with no label (guest SELinux disabled, an unlabeled workload, or a
/// privileged container) is left as this fix found it: the request is ignored and the child
/// inherits the caller's domain. That branch preserves existing behaviour rather than
/// establishing a binding, and the caller's domain is not necessarily the container init's.
pub fn exec_label(container: Option<&str>, requested: Option<&str>) -> Result<Option<String>> {
    let clean = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    let bound = match clean(container) {
        Some(bound) => bound,
        None => return Ok(None),
    };

    match clean(requested) {
        None => Ok(Some(bound)),
        Some(requested) if requested == bound => Ok(Some(bound)),
        Some(_) => bail!("ExecProcess cannot change the container SELinux label"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LABEL: &str = "system_u:system_r:unconfined_t:s0";
    #[test]
    fn exec_label_inherits_the_created_identity() {
        let bound = "system_u:system_r:container_t:s0:c42,c99";
        assert_eq!(
            super::exec_label(Some(bound), None).unwrap().as_deref(),
            Some(bound)
        );
        assert_eq!(
            super::exec_label(Some(bound), Some("   "))
                .unwrap()
                .as_deref(),
            Some(bound)
        );
        assert_eq!(
            super::exec_label(Some(bound), Some(bound))
                .unwrap()
                .as_deref(),
            Some(bound)
        );
    }

    #[test]
    fn exec_label_rejects_rebinding_to_another_identity() {
        for requested in [
            "system_u:system_r:spc_t:s0",
            "system_u:system_r:container_t:s0",
            "system_u:system_r:container_kvm_t:s0:c42,c99",
        ] {
            let err = super::exec_label(
                Some("system_u:system_r:container_t:s0:c42,c99"),
                Some(requested),
            )
            .expect_err("a different identity must not be accepted");
            assert!(
                err.to_string()
                    .contains("cannot change the container SELinux label"),
                "unexpected error: {:#}",
                err
            );
        }
    }

    #[test]
    fn exec_label_keeps_unlabeled_containers_inheriting_the_caller() {
        // Guest SELinux disabled or a privileged container: the agent holds no binding, so
        // it must neither invent one nor honour a request that tries to supply one.
        assert_eq!(super::exec_label(None, None).unwrap(), None);
        assert_eq!(super::exec_label(Some(""), Some("")).unwrap(), None);
        assert!(super::exec_label(None, Some("system_u:system_r:spc_t:s0")).is_ok());
        assert_eq!(
            super::exec_label(None, Some("system_u:system_r:spc_t:s0")).unwrap(),
            None
        );
    }

    #[test]
    fn test_is_enabled() {
        let ret = is_enabled();
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_add_mount_label() {
        let mut data = String::new();
        add_mount_label(&mut data, TEST_LABEL);
        assert_eq!(data, format!("context=\"{}\"", TEST_LABEL));

        let mut data = String::from("defaults");
        add_mount_label(&mut data, TEST_LABEL);
        assert_eq!(data, format!("defaults,context=\"{}\"", TEST_LABEL));
    }

    #[test]
    fn test_set_exec_label() {
        let ret = set_exec_label(TEST_LABEL);
        if is_enabled().unwrap() {
            assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
        } else {
            assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
        }
    }
}
