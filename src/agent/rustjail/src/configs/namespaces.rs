// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use serde;
#[macro_use]
use serde_derive;
use serde_json;

use std::collections::HashMap;
#[macro_use]
use lazy_static;

pub type NamespaceType = String;
pub type Namespaces = Vec<Namespace>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Namespace {
    #[serde(default)]
    r#type: NamespaceType,
    #[serde(default)]
    path: String,
}

pub const NEWNET: &'static str = "NEWNET";
pub const NEWPID: &'static str = "NEWPID";
pub const NEWNS: &'static str = "NEWNS";
pub const NEWUTS: &'static str = "NEWUTS";
pub const NEWUSER: &'static str = "NEWUSER";
pub const NEWCGROUP: &'static str = "NEWCGROUP";
pub const NEWIPC: &'static str = "NEWIPC";

lazy_static! {
    static ref TYPETONAME: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("pid", "pid");
        m.insert("network", "net");
        m.insert("mount", "mnt");
        m.insert("user", "user");
        m.insert("uts", "uts");
        m.insert("ipc", "ipc");
        m.insert("cgroup", "cgroup");
        m
    };
}
