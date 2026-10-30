// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use nix::mount::{self, MsFlags};
use oci_spec::runtime::Spec;
use slog::Logger;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path;

const KATA_GUEST_SANDBOX_DNS_FILE: &str = "/run/kata-containers/sandbox/resolv.conf";
const GUEST_DNS_FILE: &str = "/etc/resolv.conf";
const KATA_GUEST_SANDBOX_DIR: &str = "/run/kata-containers/sandbox";

pub fn setup_guest_hostname(hostname: &str) -> Result<()> {
    if !hostname.is_empty() {
        fs::create_dir_all(KATA_GUEST_SANDBOX_DIR)?;
        fs::write(
            path::Path::new(KATA_GUEST_SANDBOX_DIR).join("hostname"),
            format!("{hostname}\n"),
        )?;
    }
    Ok(())
}

// Validate the incoming mount against policy before replacing its source with
// a private file in the container bundle. A workload may write its own copy,
// but must not modify the sandbox template or another workload's resolver.
pub fn copy_sandbox_files(spec: &mut Spec, bundle: &path::Path) -> Result<()> {
    copy_sandbox_files_from(spec, bundle, path::Path::new(KATA_GUEST_SANDBOX_DIR))
}

fn copy_sandbox_files_from(
    spec: &mut Spec,
    bundle: &path::Path,
    sandbox: &path::Path,
) -> Result<()> {
    if let Some(mounts) = spec.mounts_mut() {
        for mount in mounts {
            for filename in ["resolv.conf", "hostname"] {
                let source = sandbox.join(filename);
                let destination = path::Path::new("/etc").join(filename);
                if mount.source().as_ref() != Some(&source)
                    || mount.destination() != &destination
                    || mount.typ().as_deref() != Some("bind")
                {
                    continue;
                }
                let target = bundle.join(filename);
                let mut input = fs::File::open(&source).context("open sandbox file template")?;
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o644)
                    .open(&target)
                    .context("create private sandbox file")?;
                std::io::copy(&mut input, &mut output).context("copy sandbox file template")?;
                mount.set_source(Some(target));
            }
        }
    }
    Ok(())
}

// Network describes a sandbox network, includings its dns
// related information.
#[derive(Debug, Default)]
pub struct Network {
    dns: Vec<String>,
}

impl Network {
    pub fn new() -> Network {
        Network { dns: Vec::new() }
    }

    pub fn set_dns(&mut self, dns: String) {
        self.dns.push(dns);
    }
}

pub fn setup_guest_dns(logger: Logger, dns_list: &[String]) -> Result<()> {
    do_setup_guest_dns(
        logger,
        dns_list,
        KATA_GUEST_SANDBOX_DNS_FILE,
        GUEST_DNS_FILE,
    )
}

fn do_setup_guest_dns(logger: Logger, dns_list: &[String], src: &str, dst: &str) -> Result<()> {
    let logger = logger.new(o!( "subsystem" => "network"));

    if dns_list.is_empty() {
        info!(
            logger,
            "Did not set sandbox DNS as DNS not received as part of request."
        );
        return Ok(());
    }

    let attr = fs::metadata(dst);
    if attr.is_err() {
        // not exists or other errors that we could not use it anymore.
        return Ok(());
    }

    if attr.unwrap().is_dir() {
        return Err(anyhow!("{} is a directory", dst));
    }

    // write DNS to file
    let content = dns_list
        .iter()
        .map(|x| x.trim())
        .collect::<Vec<&str>>()
        .join("\n");

    // make sure the src file's parent path exist.
    let file_path = path::Path::new(src);
    if let Some(p) = file_path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(src, content)?;

    // bind mount to /etc/resolv.conf
    mount::mount(Some(src), dst, Some("bind"), MsFlags::MS_BIND, None::<&str>)
        .map_err(|err| anyhow!(err).context("failed to setup guest DNS"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::mount;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use test_utils::skip_if_not_root;

    #[test]
    fn sandbox_file_copies_are_private_and_preserve_mount_options() {
        let sandbox = tempdir().unwrap();
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        let mut spec = Spec::default();
        let mut mounts = Vec::new();
        for filename in ["resolv.conf", "hostname"] {
            fs::write(sandbox.path().join(filename), "original\n").unwrap();
            mounts.push(
                oci_spec::runtime::MountBuilder::default()
                    .destination(path::Path::new("/etc").join(filename))
                    .source(sandbox.path().join(filename))
                    .typ("bind")
                    .options(vec!["rbind".to_string(), "rprivate".into(), "rw".into()])
                    .build()
                    .unwrap(),
            );
        }
        spec.set_mounts(Some(mounts));
        let mut sibling = spec.clone();
        copy_sandbox_files_from(&mut spec, first.path(), sandbox.path()).unwrap();
        copy_sandbox_files_from(&mut sibling, second.path(), sandbox.path()).unwrap();
        for (index, filename) in ["resolv.conf", "hostname"].iter().enumerate() {
            fs::write(first.path().join(filename), "changed\n").unwrap();
            assert_eq!(
                fs::read_to_string(sandbox.path().join(filename)).unwrap(),
                "original\n"
            );
            assert_eq!(
                fs::read_to_string(second.path().join(filename)).unwrap(),
                "original\n"
            );
            let mount = &spec.mounts().as_ref().unwrap()[index];
            assert_eq!(
                mount.source().as_ref().unwrap(),
                &first.path().join(filename)
            );
            assert_eq!(
                mount.options().as_ref().unwrap(),
                &vec!["rbind", "rprivate", "rw"]
            );
        }
    }

    #[test]
    fn sandbox_file_copy_preserves_explicit_mounts() {
        let sandbox = tempdir().unwrap();
        let bundle = tempdir().unwrap();
        let mount = oci_spec::runtime::MountBuilder::default()
            .destination("/etc/resolv.conf")
            .source("/custom/resolv.conf")
            .typ("bind")
            .build()
            .unwrap();
        let mut spec = Spec::default();
        spec.set_mounts(Some(vec![mount.clone()]));
        copy_sandbox_files_from(&mut spec, bundle.path(), sandbox.path()).unwrap();
        assert_eq!(spec.mounts().as_ref().unwrap(), &vec![mount]);
        assert!(!bundle.path().join("resolv.conf").exists());
    }

    #[test]
    fn sandbox_file_copy_rejects_existing_target() {
        let sandbox = tempdir().unwrap();
        let bundle = tempdir().unwrap();
        fs::write(sandbox.path().join("resolv.conf"), "template").unwrap();
        let protected = bundle.path().join("protected");
        fs::write(&protected, "preserve").unwrap();
        std::os::unix::fs::symlink(&protected, bundle.path().join("resolv.conf")).unwrap();
        let mut spec = Spec::default();
        spec.set_mounts(Some(vec![oci_spec::runtime::MountBuilder::default()
            .destination("/etc/resolv.conf")
            .source(sandbox.path().join("resolv.conf"))
            .typ("bind")
            .build()
            .unwrap()]));
        assert!(copy_sandbox_files_from(&mut spec, bundle.path(), sandbox.path()).is_err());
        assert_eq!(fs::read_to_string(protected).unwrap(), "preserve");
    }

    #[test]
    fn test_setup_guest_dns() {
        skip_if_not_root!();

        let drain = slog::Discard;
        let logger = slog::Logger::root(drain, o!());

        // create temp for /run/kata-containers/sandbox/resolv.conf
        let src_dir = tempdir().expect("failed to create tmpdir");
        let tmp = src_dir.path().join("resolv.conf");
        let src_filename = tmp.to_str().expect("failed to get resolv file filename");

        // create temp for /etc/resolv.conf
        let dst_dir = tempdir().expect("failed to create tmpdir");
        let tmp = dst_dir.path().join("resolv.conf");
        let dst_filename = tmp.to_str().expect("failed to get resolv file filename");
        {
            let _file = File::create(dst_filename).unwrap();
        }

        // test DNS
        let dns = vec![
            "nameserver 1.2.3.4".to_string(),
            "nameserver 5.6.7.8".to_string(),
        ];

        // write to /run/kata-containers/sandbox/resolv.conf
        let mut src_file = File::create(src_filename)
            .unwrap_or_else(|_| panic!("failed to create file {:?}", src_filename));
        let content = dns.join("\n");
        src_file
            .write_all(content.as_bytes())
            .expect("failed to write file contents");

        // call do_setup_guest_dns
        let result = do_setup_guest_dns(logger, &dns, src_filename, dst_filename);

        assert!(result.is_ok(), "result should be ok, but {:?}", result);

        // get content of /etc/resolv.conf
        let content = fs::read_to_string(dst_filename);
        assert!(content.is_ok());
        let content = content.unwrap();

        let expected_dns: Vec<&str> = content.split('\n').collect();

        // assert the data are the same as /run/kata-containers/sandbox/resolv.conf
        assert_eq!(dns, expected_dns);

        // umount /etc/resolv.conf
        let _ = mount::umount(dst_filename);
    }
}
