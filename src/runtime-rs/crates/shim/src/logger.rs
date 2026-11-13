// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    ffi::OsStr,
    fs::DirBuilder,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use kata_types::config::{RuntimeLogDestination, TomlConfig};

fn runtime_config_path() -> String {
    std::env::var("KATA_CONF_FILE")
        .ok()
        .filter(|path| !path.is_empty())
        .unwrap_or_default()
}

fn shim_log_path_at(log_dir: &Path, sid: &str) -> Result<PathBuf> {
    if sid.is_empty() || Path::new(sid).file_name() != Some(OsStr::new(sid)) {
        bail!("invalid sandbox id for shim log file");
    }

    Ok(log_dir.join(format!("{sid}.log")))
}

fn create_log_rotator_at(
    log_dir: &Path,
    sid: &str,
    rotation_size: u64,
    rotation_count: u32,
) -> Result<logging::FileRotator> {
    if !log_dir.is_absolute() {
        bail!("shim log directory must be an absolute path");
    }
    if rotation_size == 0 {
        bail!("shim log rotation size must be greater than zero");
    }
    if rotation_count == 0 {
        bail!("shim log rotation count must be greater than zero");
    }

    DirBuilder::new()
        .recursive(true)
        .mode(0o750)
        .create(log_dir)
        .with_context(|| format!("create shim log directory {}", log_dir.display()))?;

    let log_path = shim_log_path_at(log_dir, sid)?;
    let mut rotator = logging::FileRotator::new(&log_path)
        .with_context(|| format!("configure shim log file {}", log_path.display()))?;
    rotator
        .truncate_mode(false)
        .rotate_threshold(rotation_size)
        .rotate_count(rotation_count as usize)
        .file_mode(0o640)
        .open()
        .with_context(|| format!("open shim log file {}", log_path.display()))?;

    Ok(rotator)
}

pub(crate) fn set_logger(sid: &str, is_debug: bool) -> Result<slog_async::AsyncGuard> {
    let level = if is_debug {
        slog::Level::Debug
    } else {
        slog::Level::Info
    };

    // Logger startup precedes runtime initialization; deserialize the raw
    // configuration selected through KATA_CONF_FILE or the default search.
    let (config, _) = TomlConfig::load_raw_from_file(runtime_config_path())
        .context("load shim log destination")?;

    let destination = match config.runtime.log_destination {
        RuntimeLogDestination::Journald => logging::LogDestination::Journal,
        RuntimeLogDestination::File => {
            let rotator = create_log_rotator_at(
                Path::new(&config.runtime.log_dir),
                sid,
                config.runtime.log_rotation_size,
                config.runtime.log_rotation_count,
            )?;
            logging::LogDestination::RotatingFile(Box::new(rotator))
        }
    };

    let (logger, async_guard) =
        logging::create_logger_with_destination("kata-runtime", sid, level, destination);

    // not reset global logger when drop
    slog_scope::set_global_logger(logger).cancel_reset();

    let level = if is_debug {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    slog_stdlog::init_with_level(level).context(format!("init with level {level}"))?;

    // Regist component loggers for later use, there loggers are set directly by configuration
    logging::register_component_logger("agent");
    logging::register_component_logger("runtimes");
    logging::register_component_logger("hypervisor");

    Ok(async_guard)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn log_path_cannot_escape_configured_directory() {
        let dir = Path::new("/var/log/kata-containers");
        assert_eq!(
            shim_log_path_at(dir, "sandbox-id").unwrap(),
            dir.join("sandbox-id.log")
        );
        for sid in ["", ".", "..", "../escape", "dir/name"] {
            assert!(shim_log_path_at(dir, sid).is_err(), "accepted {:?}", sid);
        }
        assert!(create_log_rotator_at(Path::new("relative"), "sandbox-id", 1, 1).is_err());
    }

    #[test]
    fn file_destination_creates_and_rotates_log() {
        let temp_dir = tempfile::tempdir().unwrap();
        {
            let mut file = create_log_rotator_at(temp_dir.path(), "sandbox-id", 6, 1).unwrap();
            writeln!(file, "first").unwrap();
            file.flush().unwrap();
        }
        {
            let mut file = create_log_rotator_at(temp_dir.path(), "sandbox-id", 6, 1).unwrap();
            writeln!(file, "second").unwrap();
            file.flush().unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("sandbox-id.log")).unwrap(),
            ""
        );
        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("sandbox-id.log.1")).unwrap(),
            "second\n"
        );
    }

    #[test]
    fn file_destination_rejects_invalid_rotation_settings() {
        let temp_dir = tempfile::tempdir().unwrap();
        assert!(create_log_rotator_at(temp_dir.path(), "sandbox-id", 0, 1).is_err());
        assert!(create_log_rotator_at(temp_dir.path(), "sandbox-id", 1, 0).is_err());
    }
}
