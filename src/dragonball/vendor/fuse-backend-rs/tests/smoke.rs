// Copyright 2020-2022 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(all(feature = "fusedev", target_os = "linux"))]
#[macro_use]
extern crate log;

mod example;

#[cfg(all(feature = "fusedev", target_os = "linux"))]
mod fusedev_tests {
    use std::io::Result;
    use std::path::Path;
    use std::process::Command;

    use vmm_sys_util::tempdir::TempDir;

    use crate::example::passthroughfs;

    fn validate_two_git_directory(src: &str, dest: &str) -> bool {
        let src_files =
            exec(format!("cd {}; git ls-files;cd - > /dev/null", src).as_str()).unwrap();
        let dest_files =
            exec(format!("cd {}; git ls-files;cd - > /dev/null", dest).as_str()).unwrap();
        if src_files != dest_files {
            error!(
                "src {}:\n{}\ndest {}:\n{}",
                src, src_files, dest, dest_files
            );
            return false;
        }

        let src_md5 = exec(
            format!(
                "cd {}; git ls-files --recurse-submodules | grep -v rust-vmm-ci | xargs md5sum; cd - > /dev/null",
                src
            )
            .as_str(),
        )
        .unwrap();
        let dest_md5 = exec(
            format!(
                "cd {}; git ls-files --recurse-submodules | grep -v rust-vmm-ci | xargs md5sum; cd - > /dev/null",
                dest
            )
            .as_str(),
        )
        .unwrap();
        if src_md5 != dest_md5 {
            error!("src {}:\n{}\ndest {}:\n{}", src, src_md5, dest, dest_md5,);
            return false;
        }

        return true;
    }

    fn exec(cmd: &str) -> Result<String> {
        debug!("exec: {}", cmd);
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .env("RUST_BACKTRACE", "1")
            .output()?;

        if !output.status.success() || output.stderr.len() > 0 {
            let msg = std::str::from_utf8(&output.stderr).unwrap();
            panic!("exec failed: {}: {}", cmd, msg);
        }
        let stdout = std::str::from_utf8(&output.stdout).unwrap();

        return Ok(stdout.to_string());
    }

    #[test]
    #[ignore] // it depends on privileged mode to pass through /dev/fuse
    fn integration_test_tree_gitrepo() -> Result<()> {
        // test the fuse-rs repository
        let src = Path::new(".").canonicalize().unwrap();
        let src_dir = src.to_str().unwrap();
        let tmp_dir = TempDir::new().unwrap();
        let mnt_dir = tmp_dir.as_path().to_str().unwrap();
        info!(
            "test passthroughfs src {:?} mountpoint {}",
            src_dir, mnt_dir
        );

        let mut daemon = passthroughfs::Daemon::new(src_dir, mnt_dir, 2).unwrap();
        daemon.mount().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert!(validate_two_git_directory(src_dir, mnt_dir));
        daemon.umount().unwrap();
        Ok(())
    }
}
