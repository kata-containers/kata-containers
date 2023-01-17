// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::container::Container;
use liboci_cli::Kill;
use nix::sys::signal::Signal;
use slog::{info, Logger};
use std::{convert::TryFrom, path::Path, str::FromStr};

pub fn run(opts: Kill, state_root: &Path, logger: &Logger) -> Result<()> {
    let container_id = &opts.container_id;
    let container = Container::load(state_root, container_id)?;
    let sig = parse_signal(&opts.signal)?;

    let all = opts.all;
    container.kill(sig, all)?;

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
