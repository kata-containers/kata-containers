// Copyright 2020-2022 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(all(feature = "fusedev", target_os = "macos"))]
#[macro_use]
extern crate log;

mod example;

#[cfg(all(feature = "fusedev", target_os = "macos"))]
mod macfuse_tests {
    use std::io::Result;
    use std::process::Command;

    use vmm_sys_util::tempdir::TempDir;

    use crate::example::macfuse;

    fn validate_hello_file(dest: &str) -> bool {
        let files = exec(format!("cd {}; ls -la .;cd - > /dev/null", dest).as_str()).unwrap();
        if files.find("hello").is_none() {
            error!("files {}:\n not include hello \n", files);
            return false;
        }
        println!("files: {}", files);

        let content = exec(format!("cat {}/hello;", dest).as_str()).unwrap();
        if !content.eq("hello, fuse") {
            error!("content {}:\n is not right\n", content);
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
    fn integration_test_macfuse_hello() -> Result<()> {
        // test the fuse-rs repository
        let tmp_dir = TempDir::new().unwrap();
        let mnt_dir = tmp_dir.as_path().to_str().unwrap();
        info!("test macfuse mountpoint {}", mnt_dir);

        let mut daemon = macfuse::Daemon::new(mnt_dir, 2).unwrap();
        daemon.mount().unwrap();
        assert!(validate_hello_file(mnt_dir));
        daemon.umount().unwrap();
        Ok(())
    }
}
