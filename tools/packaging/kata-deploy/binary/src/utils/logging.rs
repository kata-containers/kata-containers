// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};

pub fn debug_log_file_contents(label: &str, path: &Path) {
    if !log::log_enabled!(log::Level::Debug) {
        return;
    }

    match fs::read_to_string(path) {
        Ok(content) => log::debug!("{} ({}):\n{}", label, path.display(), content),
        Err(e) => log::debug!("Unable to read {} ({}): {}", label, path.display(), e),
    }
}

pub fn debug_log_directory_file_contents(label: &str, dir: &Path) {
    if !log::log_enabled!(log::Level::Debug) {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::debug!(
                "Unable to read {} directory ({}): {}",
                label,
                dir.display(),
                e
            );
            return;
        }
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file())
        .collect();
    files.sort();

    for file in files {
        debug_log_file_contents(label, &file);
    }
}
