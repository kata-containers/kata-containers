// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Mounts one overlay for kata-agent.
//!
//! Usage: kata-overlay-mount <target> [<option>...]
//!
//! overlayfs keeps the credentials of the process that mounted it and checks
//! every access to a layer with them, mmap included. Under guest SELinux this
//! binary runs in its own domain, so the overlay holds that domain instead of
//! the agent's, and the agent cannot map container content.

use std::path::PathBuf;
use std::process::ExitCode;

use kata_sys_util::mount::Mounter;
use kata_types::mount::Mount;

const OVERLAY_TYPE: &str = "overlay";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(target) = args.next() else {
        eprintln!("usage: kata-overlay-mount <target> [<option>...]");
        return ExitCode::FAILURE;
    };

    let mount = Mount {
        source: OVERLAY_TYPE.to_string(),
        destination: PathBuf::from(&target),
        fs_type: OVERLAY_TYPE.to_string(),
        options: args.collect(),
        ..Default::default()
    };

    if let Err(err) = mount.mount(&target) {
        eprintln!("kata-overlay-mount: {target}: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
