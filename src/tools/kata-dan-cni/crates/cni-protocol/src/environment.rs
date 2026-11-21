// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use crate::{CniCommand, CniError, CniErrorCode};
use std::collections::BTreeMap;
use std::env::{self, VarError};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CniEnvironment {
    pub command: CniCommand,
    pub container_id: Option<String>,
    pub netns: Option<String>,
    pub if_name: Option<String>,
    pub cni_args: Option<String>,
    pub cni_path: Vec<PathBuf>,
}

impl CniEnvironment {
    /// Read an entry from CNI_ARGS.
    pub fn cni_arg(&self, key: &str) -> Option<&str> {
        self.cni_args.as_deref()?.split(';').find_map(|argument| {
            let (name, value) = argument.split_once('=')?;
            (name == key).then_some(value)
        })
    }

    pub fn from_process() -> Result<Self, CniError> {
        let mut values = BTreeMap::new();
        for name in [
            "CNI_COMMAND",
            "CNI_CONTAINERID",
            "CNI_NETNS",
            "CNI_IFNAME",
            "CNI_ARGS",
            "CNI_PATH",
        ] {
            match env::var(name) {
                Ok(value) => {
                    values.insert(name.to_owned(), value);
                }
                Err(VarError::NotPresent) => {}
                Err(VarError::NotUnicode(_)) => {
                    return Err(CniError::new(
                        CniErrorCode::InvalidEnvironment,
                        "invalid CNI environment variable",
                        format!("{name} must contain valid UTF-8"),
                    ));
                }
            }
        }
        Self::from_map(&values)
    }

    pub fn from_map(values: &BTreeMap<String, String>) -> Result<Self, CniError> {
        let command_text = required(values, "CNI_COMMAND")?;
        let command = command_text.parse::<CniCommand>()?;
        if command == CniCommand::Version {
            return Ok(Self {
                command,
                container_id: None,
                netns: None,
                if_name: None,
                cni_args: None,
                cni_path: Vec::new(),
            });
        }

        let container_id = required(values, "CNI_CONTAINERID")?.to_owned();
        let if_name = required(values, "CNI_IFNAME")?.to_owned();
        validate_attachment_identity(&container_id, &if_name)?;
        let netns = optional_non_empty(values, "CNI_NETNS").map(ToOwned::to_owned);
        if matches!(command, CniCommand::Add | CniCommand::Check) && netns.is_none() {
            return Err(missing_environment("CNI_NETNS"));
        }
        let cni_path = optional_non_empty(values, "CNI_PATH")
            .map(env::split_paths)
            .map(Iterator::collect)
            .unwrap_or_default();

        Ok(Self {
            command,
            container_id: Some(container_id),
            netns,
            if_name: Some(if_name),
            cni_args: optional_non_empty(values, "CNI_ARGS").map(ToOwned::to_owned),
            cni_path,
        })
    }
}

fn required<'a>(
    values: &'a BTreeMap<String, String>,
    name: &'static str,
) -> Result<&'a str, CniError> {
    optional_non_empty(values, name).ok_or_else(|| missing_environment(name))
}

fn optional_non_empty<'a>(values: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    values
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

fn missing_environment(name: &'static str) -> CniError {
    CniError::new(
        CniErrorCode::InvalidEnvironment,
        "missing CNI environment variable",
        name,
    )
}

pub fn validate_attachment_identity(container_id: &str, if_name: &str) -> Result<(), CniError> {
    validate_container_id(container_id)?;
    validate_if_name(if_name)
}

fn validate_container_id(value: &str) -> Result<(), CniError> {
    let valid = value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'));
    if !valid {
        return Err(CniError::new(
            CniErrorCode::InvalidEnvironment,
            "invalid CNI container ID",
            "CNI_CONTAINERID must start with an ASCII alphanumeric and contain only ASCII alphanumerics, '_', '.', or '-'",
        ));
    }
    Ok(())
}

fn validate_if_name(value: &str) -> Result<(), CniError> {
    if value.len() > 15
        || matches!(value, "." | "..")
        || value
            .chars()
            .any(|character| matches!(character, '/' | ':' | '\0') || character.is_whitespace())
    {
        return Err(CniError::new(
            CniErrorCode::InvalidEnvironment,
            "invalid CNI interface name",
            "CNI_IFNAME must be a valid Linux interface name of at most 15 bytes",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{validate_attachment_identity, CniCommand, CniEnvironment, CniErrorCode};
    use std::collections::BTreeMap;

    fn environment(command: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("CNI_COMMAND".into(), command.into()),
            ("CNI_CONTAINERID".into(), "sandbox-01".into()),
            ("CNI_NETNS".into(), "/run/netns/sandbox-01".into()),
            ("CNI_IFNAME".into(), "eth0".into()),
        ])
    }

    #[test]
    fn required_environment() {
        for command in ["ADD", "CHECK"] {
            let mut values = environment(command);
            assert!(CniEnvironment::from_map(&values).is_ok());
            for required in ["CNI_CONTAINERID", "CNI_IFNAME", "CNI_NETNS"] {
                let saved = values.insert(required.into(), String::new());
                let error = CniEnvironment::from_map(&values).expect_err("required environment");
                assert_eq!(error.code(), CniErrorCode::InvalidEnvironment.value());
                assert_eq!(error.response().details, required);
                values.insert(required.into(), saved.expect("fixture field"));
            }
        }
        let mut values = environment("DEL");
        values.remove("CNI_NETNS");
        let parsed = CniEnvironment::from_map(&values).expect("DEL after namespace removal");
        assert_eq!(parsed.command, CniCommand::Del);
        assert_eq!(parsed.netns, None);

        let parsed =
            CniEnvironment::from_map(&BTreeMap::from([("CNI_COMMAND".into(), "VERSION".into())]))
                .expect("VERSION needs no attachment environment");
        assert_eq!(parsed.container_id, None);
        assert_eq!(parsed.if_name, None);
    }

    #[test]
    fn reject_unsafe_names() {
        for container in ["../sandbox", "/tmp/sandbox", ".hidden", "pod name", "沙箱"] {
            assert!(
                validate_attachment_identity(container, "eth0").is_err(),
                "{container}"
            );
        }
        for interface in ["../eth0", "eth:0", "eth 0", "abcdefghijklmnop", ".", ".."] {
            assert!(
                validate_attachment_identity("sandbox-1", interface).is_err(),
                "{interface}"
            );
        }
        assert!(validate_attachment_identity("sandbox_a.b-01", "net1").is_ok());
    }
}
