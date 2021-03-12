// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use anyhow::{anyhow, Context, Result};
use libc::gid_t;
use libc::uid_t;
use std::fs::File;
use std::io::{BufRead, BufReader};

const PASSWD_FILE: &str = "/etc/passwd";

// An entry from /etc/passwd
#[derive(Debug, PartialEq, PartialOrd)]
pub struct PasswdEntry {
    // username
    pub name: String,
    // user password
    pub passwd: String,
    // user id
    pub uid: uid_t,
    // group id
    pub gid: gid_t,
    // user Information
    pub gecos: String,
    // home directory
    pub dir: String,
    // User's Shell
    pub shell: String,
}

// get an entry for a given `uid` from `/etc/passwd`
fn get_entry_by_uid(uid: uid_t, path: &str) -> Result<PasswdEntry> {
    let file = File::open(path).with_context(|| format!("open file {}", path))?;
    let mut reader = BufReader::new(file);

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return Err(anyhow!(format!("file {} is empty", path))),
            Ok(_) => (),
            Err(e) => {
                return Err(anyhow!(format!(
                    "failed to read file {} with {:?}",
                    path, e
                )))
            }
        }

        if line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split(':').map(|part| part.trim()).collect();
        if parts.len() != 7 {
            continue;
        }

        match parts[2].parse() {
            Err(_e) => continue,
            Ok(new_uid) => {
                if uid != new_uid {
                    continue;
                }

                let entry = PasswdEntry {
                    name: parts[0].to_string(),
                    passwd: parts[1].to_string(),
                    uid: new_uid,
                    gid: parts[3].parse().unwrap_or(0),
                    gecos: parts[4].to_string(),
                    dir: parts[5].to_string(),
                    shell: parts[6].to_string(),
                };

                return Ok(entry);
            }
        }
    }
}

pub fn home_dir(uid: uid_t) -> Result<String> {
    get_entry_by_uid(uid, PASSWD_FILE).map(|entry| entry.dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::Builder;

    #[test]
    fn test_get_entry_by_uid() {
        let tmpdir = Builder::new().tempdir().unwrap();
        let tmpdir_path = tmpdir.path().to_str().unwrap();
        let temp_passwd = format!("{}/passwd", tmpdir_path);

        let mut tempf = File::create(temp_passwd.as_str()).unwrap();
        writeln!(tempf, "root:x:0:0:root:/root0:/bin/bash").unwrap();
        writeln!(tempf, "root:x:1:0:root:/root1:/bin/bash").unwrap();
        writeln!(tempf, "#root:x:1:0:root:/rootx:/bin/bash").unwrap();
        writeln!(tempf, "root:x:2:0:root:/root2:/bin/bash").unwrap();
        writeln!(tempf, "root:x:3:0:root:/root3").unwrap();
        writeln!(tempf, "root:x:3:0:root:/root3:/bin/bash").unwrap();

        let entry = get_entry_by_uid(0, temp_passwd.as_str()).unwrap();
        assert_eq!(entry.dir.as_str(), "/root0");

        let entry = get_entry_by_uid(1, temp_passwd.as_str()).unwrap();
        assert_eq!(entry.dir.as_str(), "/root1");

        let entry = get_entry_by_uid(2, temp_passwd.as_str()).unwrap();
        assert_eq!(entry.dir.as_str(), "/root2");

        let entry = get_entry_by_uid(3, temp_passwd.as_str()).unwrap();
        assert_eq!(entry.dir.as_str(), "/root3");
    }
}
