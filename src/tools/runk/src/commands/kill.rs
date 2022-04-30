// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::Kill;
use anyhow::{anyhow, Result};
use libcontainer::status::{self, get_current_container_state, Status};
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use oci::ContainerState;
use slog::{info, Logger};
use std::{convert::TryFrom, path::Path, str::FromStr};

pub fn run(opts: Kill, state_root: &Path, logger: &Logger) -> Result<()> {
    let container_id = &opts.container_id;
    let status = Status::load(state_root, container_id)?;
    let current_state = get_current_container_state(&status)?;
    let sig = parse_signal(&opts.signal)?;

    // TODO: liboci-cli does not support --all option for kill command.
    // After liboci-cli supports the option, we will change the following code.
    // as a workaround we use a custom Kill command.
    let all = opts.all;
    if all {
        let pids = status::get_all_pid(&status.cgroup_manager)?;
        for pid in pids {
            if !status::is_process_running(pid)? {
                continue;
            }
            kill(pid, sig)?;
        }
    } else {
        if current_state == ContainerState::Stopped {
            return Err(anyhow!("container {} not running", container_id));
        }

        let p = Pid::from_raw(status.pid);
        if status::is_process_running(p)? {
            kill(p, sig)?;
        }
    }

    info!(&logger, "kill command finished successfully");

    Ok(())
}

fn parse_signal(signal: &str) -> Result<Signal> {
    if let Ok(num) = signal.parse::<i32>() {
        return Ok(Signal::try_from(num)?);
    }

    let mut signal_upper = signal.to_uppercase();
    if !signal_upper.starts_with("SIG") {
        signal_upper = "SIG".to_string() + &signal_upper;
    }

    Ok(Signal::from_str(&signal_upper)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::signal::Signal;

    #[test]
    fn test_parse_signal() {
        assert_eq!(Signal::SIGHUP, parse_signal("1").unwrap());
        assert_eq!(Signal::SIGHUP, parse_signal("sighup").unwrap());
        assert_eq!(Signal::SIGHUP, parse_signal("hup").unwrap());
        assert_eq!(Signal::SIGHUP, parse_signal("SIGHUP").unwrap());
        assert_eq!(Signal::SIGHUP, parse_signal("HUP").unwrap());

        assert_eq!(Signal::SIGKILL, parse_signal("9").unwrap());
        assert_eq!(Signal::SIGKILL, parse_signal("sigkill").unwrap());
        assert_eq!(Signal::SIGKILL, parse_signal("kill").unwrap());
        assert_eq!(Signal::SIGKILL, parse_signal("SIGKILL").unwrap());
        assert_eq!(Signal::SIGKILL, parse_signal("KILL").unwrap());
    }
}
