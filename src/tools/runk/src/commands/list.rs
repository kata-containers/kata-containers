// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use super::state::get_container_state_name;
use anyhow::Result;
use libcontainer::status::{get_current_container_state, Status};
use liboci_cli::List;
use oci::ContainerState;
use slog::{info, Logger};
use std::{fs, os::unix::prelude::MetadataExt, path::Path};
use std::{io, io::Write};
use tabwriter::TabWriter;
use users::get_user_by_uid;

pub fn run(_: List, root: &Path, logger: &Logger) -> Result<()> {
    let mut content = String::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        // Possibly race with runk delete, so continue loop when any error occurs below
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if !metadata.is_dir() {
            continue;
        }
        let container_id = match entry.file_name().into_string() {
            Ok(id) => id,
            Err(_) => continue,
        };
        let status = match Status::load(root, &container_id) {
            Ok(status) => status,
            Err(_) => continue,
        };
        let state = match get_current_container_state(&status) {
            Ok(state) => state,
            Err(_) => continue,
        };
        // Just like runc, pid of stopped container is 0
        let pid = match state {
            ContainerState::Stopped => 0,
            _ => status.pid,
        };
        // May replace get_user_by_uid with getpwuid(3)
        let owner = match get_user_by_uid(metadata.uid()) {
            Some(user) => String::from(user.name().to_string_lossy()),
            None => format!("#{}", metadata.uid()),
        };
        content.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            container_id,
            pid,
            get_container_state_name(state),
            status.bundle.display(),
            status.created,
            owner
        ));
    }

    let mut tab_writer = TabWriter::new(io::stdout());
    writeln!(&mut tab_writer, "ID\tPID\tSTATUS\tBUNDLE\tCREATED\tOWNER")?;
    write!(&mut tab_writer, "{}", content)?;
    tab_writer.flush()?;

    info!(&logger, "list command finished successfully");
    Ok(())
}
