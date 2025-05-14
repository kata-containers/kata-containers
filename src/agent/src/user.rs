// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use anyhow::{anyhow, Result};

/// Represents a single user account from /etc/passwd
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswdEntry {
    /// Username
    pub username: String,
    /// Password (usually 'x', indicating it's stored in /etc/shadow)
    pub password: String,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// GECOS field (typically contains user's full name)
    pub gecos: String,
    /// User's home directory path
    pub home_dir: String,
    /// User's login shell
    pub shell: String,
}

/// Errors that may occur during parsing
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Invalid UID: {0}")]
    InvalidUid(String),

    #[error("Invalid GID: {0}")]
    InvalidGid(String),

    #[error("User not found: UID {0}")]
    UserNotFound(u32),

    #[error("GID mismatch (UID={0})")]
    GidMismatch(u32),
}

/// Main structure for parsing /etc/passwd file
pub struct PasswdParser;

impl PasswdParser {
    /// Read and parse /etc/passwd file from the specified path
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<PasswdEntry>, ParserError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result?;
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            match Self::parse_line(trimmed) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    return Err(ParserError::ParseError(format!(
                        "failed to parse line {}: {}",
                        line_num + 1,
                        e
                    )))
                }
            }
        }

        Ok(entries)
    }

    /// Parse a single line from /etc/passwd
    pub fn parse_line(line: &str) -> Result<PasswdEntry, ParserError> {
        let fields: Vec<&str> = line.split(':').collect();

        // /etc/passwd should have 7 fields
        if fields.len() != 7 {
            return Err(ParserError::InvalidFormat(line.to_string()));
        }

        // Parse UID and GID
        let uid = fields[2]
            .parse::<u32>()
            .map_err(|_| ParserError::InvalidUid(fields[2].to_string()))?;

        let gid = fields[3]
            .parse::<u32>()
            .map_err(|_| ParserError::InvalidGid(fields[3].to_string()))?;

        Ok(PasswdEntry {
            username: fields[0].to_string(),
            password: fields[1].to_string(),
            uid,
            gid,
            gecos: fields[4].to_string(),
            home_dir: fields[5].to_string(),
            shell: fields[6].to_string(),
        })
    }

    /// Convert entries to a HashMap with UID as key
    pub fn entries_to_uid_map(entries: Vec<PasswdEntry>) -> HashMap<u32, PasswdEntry> {
        entries
            .into_iter()
            .map(|entry| (entry.uid, entry))
            .collect()
    }

    /// Verify if UID/GID match
    pub fn verify_uid_gid(
        uid: u32,
        gid: u32,
        entries: &HashMap<u32, PasswdEntry>,
    ) -> Result<PasswdEntry, ParserError> {
        // find user by UID
        let entry = entries.get(&uid).ok_or(ParserError::UserNotFound(uid))?;

        // verify GID
        if entry.gid != gid {
            Err(ParserError::GidMismatch(entry.uid))
        } else {
            Ok(entry.clone())
        }
    }

    /// Get user information by UID
    pub fn get_user_by_uid(
        uid: u32,
        entries: &HashMap<u32, PasswdEntry>,
    ) -> Result<&PasswdEntry, ParserError> {
        entries.get(&uid).ok_or(ParserError::UserNotFound(uid))
    }
}

/// Get user identity information from passwd file
pub fn get_user_identity(rootfs_path: &Path, uid: u32, gid: u32) -> Result<PasswdEntry> {
    let passwd_path = rootfs_path.join("etc/passwd");
    let entries = PasswdParser::parse_file(passwd_path)?;
    let uid_map = PasswdParser::entries_to_uid_map(entries);

    match PasswdParser::verify_uid_gid(uid, gid, &uid_map) {
        Ok(pe) => Ok(pe),
        Err(ParserError::GidMismatch(uid)) => {
            // If so, it'll lookup the corresponding gid for uid.
            let pe = PasswdParser::get_user_by_uid(uid, &uid_map)?;
            Ok(pe.clone())
        }
        Err(e) => Err(anyhow!("invalid uid or gid: {}", e)),
    }
}

/// Represents a single group entry from /etc/group
/// Format: <Group Name>:<Password>:<GID>:<Supplementary Groups>
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupEntry {
    /// Group Name
    pub name: String,
    /// Password, almost always 'x'
    pub password: String,
    /// GID
    pub gid: u32,
    /// User List or Supplementary Groups
    pub members: Vec<String>,
}

/// GroupData structure for efficient lookups
#[derive(Debug, Clone)]
pub struct GroupData {
    entries: Vec<GroupEntry>,
}

impl GroupData {
    /// Get additional groups for a user
    pub fn additional_gids(&self, username: &str, primary_gid: u32) -> Vec<u32> {
        // 1. primary group validation with username and primary_gid
        let in_primary_group = self
            .entries
            .iter()
            .find(|g| g.gid == primary_gid)
            .map(|g| g.members.contains(&username.to_string()))
            .unwrap_or(false);

        if !in_primary_group {
            return Vec::new();
        }

        // 2. collect user's all additional groupds
        self.entries
            .iter()
            .filter(|g| {
                // user belongs to this group
                g.members.contains(&username.to_string())
            })
            .map(|g| g.gid)
            .collect()
    }
}

/// Efficient parser for /etc/group file
pub struct GroupParser;

impl GroupParser {
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<GroupData, ParserError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result?;
            let trimmed = line.trim();

            // skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            match Self::parse_line(trimmed) {
                Ok(entry) => {
                    entries.push(entry);
                }
                Err(e) => {
                    return Err(ParserError::InvalidFormat(format!(
                        "{} (line {}: {})",
                        e,
                        line_num + 1,
                        trimmed
                    )));
                }
            }
        }

        Ok(GroupData { entries })
    }

    /// Parse a single line from /etc/group
    fn parse_line(line: &str) -> Result<GroupEntry, ParserError> {
        let mut fields = line.splitn(4, ':');
        let name = fields
            .next()
            .ok_or_else(|| ParserError::InvalidFormat("missing group name".into()))?;
        let password = fields
            .next()
            .ok_or_else(|| ParserError::InvalidFormat("missing password field".into()))?;
        let gid_str = fields
            .next()
            .ok_or_else(|| ParserError::InvalidFormat("missing GID field".into()))?;
        let members_str = fields.next().unwrap_or("");

        let gid = gid_str
            .parse::<u32>()
            .map_err(|_| ParserError::InvalidGid(gid_str.into()))?;
        // parse member list more efficiently
        let members = if members_str.is_empty() {
            Vec::new()
        } else {
            members_str
                .split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        };

        Ok(GroupEntry {
            name: name.into(),
            password: password.into(),
            gid,
            members,
        })
    }
}

/// Get additional GIDs for a user
pub fn get_additional_gids(
    rootfs_path: &Path,
    username: &str,
    primary_gid: u32,
) -> Result<Vec<u32>, ParserError> {
    let group_path = rootfs_path.join("etc/group");
    let group_data = GroupParser::parse_file(group_path)?;

    Ok(group_data.additional_gids(username, primary_gid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};

    /// create temp paths for passwd and group
    fn create_temp_rootfs(passwd: &str, group: &str) -> Result<TempDir, std::io::Error> {
        let temp_dir = tempdir()?;
        let etc_dir = temp_dir.path().join("etc");
        fs::create_dir_all(&etc_dir)?;

        // passwd
        let passwd_path = etc_dir.join("passwd");
        File::create(&passwd_path)?.write_all(passwd.as_bytes())?;

        // group
        let group_path = etc_dir.join("group");
        File::create(&group_path)?.write_all(group.as_bytes())?;

        Ok(temp_dir)
    }

    // PasswdParser tests
    #[test]
    fn test_parse_valid_passwd_line() {
        let line = "redis:x:1000:1000:Linux User,,,:/home/redis:/sbin/nologin";
        let entry = PasswdParser::parse_line(line).unwrap();

        assert_eq!(entry.username, "redis");
        assert_eq!(entry.uid, 1000);
        assert_eq!(entry.gid, 1000);
        assert_eq!(entry.gecos, "Linux User,,,");
        assert_eq!(entry.home_dir, "/home/redis");
        assert_eq!(entry.shell, "/sbin/nologin");
    }

    #[test]
    fn test_passwd_file_parsing() {
        let content = r#"
        # Comment line
        root:x:0:0:root:/root:/bin/bash
        nobody:x:65534:65534:nobody:/:/sbin/nologin
        redis:x:1000:1000:Linux User,,,:/home/redis:/sbin/nologin
        "#;

        let temp_dir = tempdir().unwrap();
        let etc_dir = temp_dir.path().join("etc");
        fs::create_dir_all(&etc_dir).unwrap();

        // write passwd file
        let passwd_path = etc_dir.join("passwd");
        let mut file = File::create(passwd_path.clone()).unwrap();
        write!(file, "{}", content).unwrap();

        let entries = PasswdParser::parse_file(passwd_path).unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].username, "root");
        assert_eq!(entries[1].uid, 65534);
        assert_eq!(entries[2].gid, 1000);
    }

    // GroupParser tests
    #[test]
    fn test_parse_valid_group_line() {
        let line = "redis:x:1000:user1,user2";
        let entry = GroupParser::parse_line(line).unwrap();

        assert_eq!(entry.name, "redis");
        assert_eq!(entry.gid, 1000);
        assert_eq!(entry.members, vec!["user1", "user2"]);
    }

    #[test]
    fn test_group_member_parsing() {
        let cases = vec![
            ("group:x:100:", vec![]),
            ("group:x:100:user1", vec!["user1"]),
            ("group:x:100:user1,user2", vec!["user1", "user2"]),
            ("group:x:100:user1,,user2", vec!["user1", "user2"]),
        ];

        for (line, expected) in cases {
            let entry = GroupParser::parse_line(line).unwrap();
            assert_eq!(entry.members, expected);
        }
    }

    // GroupData tests
    #[test]
    fn test_additional_gids_calculation() {
        let group_data = GroupData {
            entries: vec![
                GroupEntry {
                    name: "primary".into(),
                    gid: 1000,
                    members: vec!["redisuser".into()],
                    password: "x".into(),
                },
                GroupEntry {
                    name: "supp1".into(),
                    gid: 1001,
                    members: vec!["redisuser".into()],
                    password: "x".into(),
                },
                GroupEntry {
                    name: "supp2".into(),
                    gid: 1002,
                    members: vec!["otheruser".into()],
                    password: "x".into(),
                },
            ],
        };

        // Valid case
        let gids = group_data.additional_gids("redisuser", 1000);
        assert_eq!(gids, vec![1000, 1001]);

        // User not in primary group
        let gids = group_data.additional_gids("redisuser", 999);
        assert!(gids.is_empty());

        // No supplementary groups
        let gids = group_data.additional_gids("otheruser", 1002);
        assert_eq!(gids, vec![1002]);
    }

    // integration tests for both user identity and additional gids
    #[test]
    fn test_user_identity_and_additional_gids() {
        // prepare contents
        let passwd_content = "redis:x:1000:1000:Linux User,,,:/home/redis:/sbin/nologin";
        let group_content = r#"
        primary:x:999:testuser
        redis:x:1000:redis
        "#;

        // create rootfs
        let temp_dir = create_temp_rootfs(passwd_content, group_content).unwrap();
        let rootfs_path = PathBuf::from(temp_dir.path());

        // test user identity
        let user = get_user_identity(&rootfs_path, 1000, 0).unwrap();
        assert_eq!(user.username, "redis");
        assert_eq!(user.uid, 1000);
        assert_eq!(user.gid, 1000);

        // test additional gids
        let gids = get_additional_gids(&rootfs_path, "redis", 1000).unwrap();
        assert_eq!(gids, vec![1000]);
    }
}
