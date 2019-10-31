// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

// looks like we can use caps to manipulate capabilities
// conveniently, use caps to do it directly.. maybe

use lazy_static;

use crate::errors::*;
use caps::{self, CapSet, Capability, CapsHashSet};
use protocols::oci::LinuxCapabilities;
use slog::Logger;
use std::collections::HashMap;

lazy_static! {
    pub static ref CAPSMAP: HashMap<String, Capability> = {
        let mut m = HashMap::new();
        m.insert("CAP_CHOWN".to_string(), Capability::CAP_CHOWN);
        m.insert("CAP_DAC_OVERRIDE".to_string(), Capability::CAP_DAC_OVERRIDE);
        m.insert(
            "CAP_DAC_READ_SEARCH".to_string(),
            Capability::CAP_DAC_READ_SEARCH,
        );
        m.insert("CAP_FOWNER".to_string(), Capability::CAP_FOWNER);
        m.insert("CAP_FSETID".to_string(), Capability::CAP_FSETID);
        m.insert("CAP_KILL".to_string(), Capability::CAP_KILL);
        m.insert("CAP_SETGID".to_string(), Capability::CAP_SETGID);
        m.insert("CAP_SETUID".to_string(), Capability::CAP_SETUID);
        m.insert("CAP_SETPCAP".to_string(), Capability::CAP_SETPCAP);
        m.insert(
            "CAP_LINUX_IMMUTABLE".to_string(),
            Capability::CAP_LINUX_IMMUTABLE,
        );
        m.insert(
            "CAP_NET_BIND_SERVICE".to_string(),
            Capability::CAP_NET_BIND_SERVICE,
        );
        m.insert(
            "CAP_NET_BROADCAST".to_string(),
            Capability::CAP_NET_BROADCAST,
        );
        m.insert("CAP_NET_ADMIN".to_string(), Capability::CAP_NET_ADMIN);
        m.insert("CAP_NET_RAW".to_string(), Capability::CAP_NET_RAW);
        m.insert("CAP_IPC_LOCK".to_string(), Capability::CAP_IPC_LOCK);
        m.insert("CAP_IPC_OWNER".to_string(), Capability::CAP_IPC_OWNER);
        m.insert("CAP_SYS_MODULE".to_string(), Capability::CAP_SYS_MODULE);
        m.insert("CAP_SYS_RAWIO".to_string(), Capability::CAP_SYS_RAWIO);
        m.insert("CAP_SYS_CHROOT".to_string(), Capability::CAP_SYS_CHROOT);
        m.insert("CAP_SYS_PTRACE".to_string(), Capability::CAP_SYS_PTRACE);
        m.insert("CAP_SYS_PACCT".to_string(), Capability::CAP_SYS_PACCT);
        m.insert("CAP_SYS_ADMIN".to_string(), Capability::CAP_SYS_ADMIN);
        m.insert("CAP_SYS_BOOT".to_string(), Capability::CAP_SYS_BOOT);
        m.insert("CAP_SYS_NICE".to_string(), Capability::CAP_SYS_NICE);
        m.insert("CAP_SYS_RESOURCE".to_string(), Capability::CAP_SYS_RESOURCE);
        m.insert("CAP_SYS_TIME".to_string(), Capability::CAP_SYS_TIME);
        m.insert(
            "CAP_SYS_TTY_CONFIG".to_string(),
            Capability::CAP_SYS_TTY_CONFIG,
        );
        m.insert("CAP_MKNOD".to_string(), Capability::CAP_MKNOD);
        m.insert("CAP_LEASE".to_string(), Capability::CAP_LEASE);
        m.insert("CAP_AUDIT_WRITE".to_string(), Capability::CAP_AUDIT_WRITE);
        m.insert("CAP_AUDIT_CONTROL".to_string(), Capability::CAP_AUDIT_WRITE);
        m.insert("CAP_SETFCAP".to_string(), Capability::CAP_SETFCAP);
        m.insert("CAP_MAC_OVERRIDE".to_string(), Capability::CAP_MAC_OVERRIDE);
        m.insert("CAP_SYSLOG".to_string(), Capability::CAP_SYSLOG);
        m.insert("CAP_WAKE_ALARM".to_string(), Capability::CAP_WAKE_ALARM);
        m.insert(
            "CAP_BLOCK_SUSPEND".to_string(),
            Capability::CAP_BLOCK_SUSPEND,
        );
        m.insert("CAP_AUDIT_READ".to_string(), Capability::CAP_AUDIT_READ);
        m
    };
}

fn to_capshashset(logger: &Logger, caps: &[String]) -> CapsHashSet {
    let mut r = CapsHashSet::new();

    for cap in caps.iter() {
        let c = CAPSMAP.get(cap);

        if c.is_none() {
            warn!(logger, "{} is not a cap", cap);
            continue;
        }

        r.insert(*c.unwrap());
    }

    r
}

pub fn reset_effective() -> Result<()> {
    caps::set(None, CapSet::Effective, caps::all())?;
    Ok(())
}

pub fn drop_priviledges(logger: &Logger, caps: &LinuxCapabilities) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "capabilities"));

    let all = caps::all();

    for c in all.difference(&to_capshashset(&logger, caps.Bounding.as_ref())) {
        caps::drop(None, CapSet::Bounding, *c)?;
    }

    caps::set(
        None,
        CapSet::Effective,
        to_capshashset(&logger, caps.Effective.as_ref()),
    )?;
    caps::set(
        None,
        CapSet::Permitted,
        to_capshashset(&logger, caps.Permitted.as_ref()),
    )?;
    caps::set(
        None,
        CapSet::Inheritable,
        to_capshashset(&logger, caps.Inheritable.as_ref()),
    )?;

    if let Err(_) = caps::set(
        None,
        CapSet::Ambient,
        to_capshashset(&logger, caps.Ambient.as_ref()),
    ) {
        warn!(logger, "failed to set ambient capability");
    }

    Ok(())
}
