// Copyright (c) 2024 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub struct Qmp {
    qmp: qapi::Qmp<qapi::Stream<BufReader<UnixStream>, UnixStream>>,
}

impl Qmp {
    pub fn new(qmp_sock_path: &str) -> Result<Self> {
        let stream = UnixStream::connect(qmp_sock_path)?;

        // Set the read timeout to protect runtime-rs from blocking forever
        // trying to set up QMP connection if qemu fails to launch.  The exact
        // value is a matter of judegement.  Setting it too long would risk
        // being ineffective since container runtime would timeout first anyway
        // (containerd's task creation timeout is 2 s by default).  OTOH
        // setting it too short would risk interfering with a normal launch,
        // perhaps just seeing some delay due to a heavily loaded host.
        stream.set_read_timeout(Some(Duration::from_millis(250)))?;

        let mut qmp = Qmp {
            qmp: qapi::Qmp::new(qapi::Stream::new(
                BufReader::new(stream.try_clone()?),
                stream,
            )),
        };

        let info = qmp.qmp.handshake()?;
        info!(sl!(), "QMP initialized: {:#?}", info);

        Ok(qmp)
    }
}
