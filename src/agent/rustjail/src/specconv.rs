// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use protocols::oci::Spec;
// use crate::configs::namespaces;
// use crate::configs::device::Device;

#[derive(Debug)]
pub struct CreateOpts {
    pub cgroup_name: String,
    pub use_systemd_cgroup: bool,
    pub no_pivot_root: bool,
    pub no_new_keyring: bool,
    pub spec: Option<Spec>,
    pub rootless_euid: bool,
    pub rootless_cgroup: bool,
}
/*
const WILDCARD: i32 = -1;

lazy_static! {
    static ref NAEMSPACEMAPPING: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert(oci::PIDNAMESPACE, namespaces::NEWPID);
        m.insert(oci::NETWORKNAMESPACE, namespaces::NEWNET);
        m.insert(oci::UTSNAMESPACE, namespaces::NEWUTS);
        m.insert(oci::MOUNTNAMESPACE, namespaces::NEWNS);
        m.insert(oci::IPCNAMESPACE, namespaces::NEWIPC);
        m.insert(oci::USERNAMESPACE, namespaces::NEWUSER);
        m.insert(oci::CGROUPNAMESPACE, namespaces::NEWCGROUP);
        m
    };

    static ref MOUNTPROPAGATIONMAPPING: HashMap<&'static str, MsFlags> = {
        let mut m = HashMap::new();
        m.insert("rprivate", MsFlags::MS_PRIVATE | MsFlags::MS_REC);
        m.insert("private", MsFlags::MS_PRIVATE);
        m.insert("rslave", MsFlags::MS_SLAVE | MsFlags::MS_REC);
        m.insert("slave", MsFlags::MS_SLAVE);
        m.insert("rshared", MsFlags::MS_SHARED | MsFlags::MS_REC);
        m.insert("shared", MsFlags::MS_SHARED);
        m.insert("runbindable", MsFlags::MS_UNBINDABLE | MsFlags::MS_REC);
        m.insert("unbindable", MsFlags::MS_UNBINDABLE);
        m
    };

    static ref ALLOWED_DEVICES: Vec<Device> = {
        let mut m = Vec::new();
        m.push(Device {
            r#type: 'c',
            major: WILDCARD,
            minor: WILDCARD,
            permissions: "m",
            allow: true,
        });

        m.push(Device {
            r#type: 'b',
            major: WILDCARD,
            minor: WILDCARD,
            permissions: "m",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: "/dev/null".to_string(),
            major: 1,
            minor: 3,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from("/dev/random"),
            major: 1,
            minor: 8,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from("/dev/full"),
            major: 1,
            minor: 7,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from("/dev/tty"),
            major: 5,
            minor: 0,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from("/dev/zero"),
            major: 1,
            minor: 5,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from("/dev/urandom"),
            major: 1,
            minor: 9,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from("/dev/console"),
            major: 5,
            minor: 1,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from(""),
            major: 136,
            minor: WILDCARD,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from(""),
            major: 5,
            minor: 2,
            permissions: "rwm",
            allow: true,
        });

        m.push(Device {
            r#type: 'c',
            path: String::from(""),
            major: 10,
            minor: 200,
            permissions: "rwm",
            allow: true,
        });
        m
    };
}
*/
