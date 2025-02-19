// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use std::collections::HashMap;

pub const OCI_SPEC_CONFIG_FILE_NAME: &str = "config.json";
pub const OCI_MOUNT_BIND_TYPE: &str = "bind";
pub const PIDNAMESPACE: &str = "pid";
pub const NETWORKNAMESPACE: &str = "network";
pub const MOUNTNAMESPACE: &str = "mount";
pub const IPCNAMESPACE: &str = "ipc";
pub const USERNAMESPACE: &str = "user";
pub const UTSNAMESPACE: &str = "uts";
pub const CGROUPNAMESPACE: &str = "cgroup";

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContainerState {
    Creating,
    Created,
    Running,
    Stopped,
    Paused,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct State {
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "ociVersion"
    )]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub status: ContainerState,
    #[serde(default)]
    pub pid: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bundle: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_state() {
        let data = r#"{
            "ociVersion": "0.2.0",
            "id": "oci-container1",
            "status": "running",
            "pid": 4422,
            "bundle": "/containers/redis",
            "annotations": {
                "myKey": "myValue"
            }
        }"#;
        let expected = State {
            version: "0.2.0".to_string(),
            id: "oci-container1".to_string(),
            status: ContainerState::Running,
            pid: 4422,
            bundle: "/containers/redis".to_string(),
            annotations: [("myKey".to_string(), "myValue".to_string())]
                .iter()
                .cloned()
                .collect(),
        };

        let current: crate::State = serde_json::from_str(data).unwrap();
        assert_eq!(expected, current);
    }
}
