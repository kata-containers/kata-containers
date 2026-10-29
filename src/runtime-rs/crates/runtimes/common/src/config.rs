// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use hypervisor::Param;
use kata_sys_util::spec::SHIM_RUNTIME_OPTIONS_FILE;
use kata_types::{annotations::Annotation, config::TomlConfig};
use protobuf::Message as ProtobufMessage;
use slog::info;
use tracing::instrument;

const MAX_STARTUP_OPTIONS_SIZE: u64 = 1024 * 1024;

fn read_startup_envelope(reader: impl Read) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    reader
        .take(MAX_STARTUP_OPTIONS_SIZE + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_STARTUP_OPTIONS_SIZE {
        return Err(anyhow!("shim startup runtime options exceed size limit"));
    }
    Ok(data)
}

fn startup_options_payload(data: &[u8]) -> Result<Option<Vec<u8>>> {
    let envelope = protobuf::well_known_types::any::Any::parse_from_bytes(data)
        .context("decode shim startup runtime options envelope")?;
    // BootstrapParams fields 1/2 collide with Any's type_url/value. Modern
    // containerd Command(start) also emits bootstrap-specific metadata (4-7).
    // Do not treat successful decoding of those colliding fields as an Any.
    let bootstrap = envelope
        .special_fields
        .unknown_fields()
        .iter()
        .any(|(number, _)| (3..=9).contains(&number));
    if bootstrap {
        let params = protocols::bootstrap::BootstrapParams::parse_from_bytes(data)
            .context("decode shim bootstrap parameters")?;
        if params.instance_id.is_empty() || params.namespace.is_empty() {
            return Err(anyhow!(
                "shim bootstrap parameters require instance ID and namespace"
            ));
        }
        let mut payload = None;
        for extension in &params.extensions {
            if let Some(value) = extension.value.as_ref() {
                if is_runtime_options_type(&value.type_url) {
                    if payload.is_some() {
                        return Err(anyhow!(
                            "duplicate runtime options in shim bootstrap parameters"
                        ));
                    }
                    validate_runtime_options(&value.value)?;
                    payload = Some(value.value.clone());
                }
            }
        }
        return Ok(payload);
    }
    if envelope.type_url.is_empty() && envelope.value.is_empty() {
        return Ok(None);
    }
    if !is_runtime_options_type(&envelope.type_url) {
        return Err(anyhow!(
            "unexpected shim runtime options type {:?}",
            envelope.type_url
        ));
    }
    validate_runtime_options(&envelope.value)?;
    Ok(Some(envelope.value))
}

fn is_runtime_options_type(type_url: &str) -> bool {
    matches!(
        type_url,
        "runtimeoptions.v1.Options" | "type.googleapis.com/runtimeoptions.v1.Options"
    )
}

fn validate_runtime_options(payload: &[u8]) -> Result<()> {
    protocols::runtimeoptions::Options::parse_from_bytes(payload)
        .context("decode shim startup runtime options")?;
    Ok(())
}

/// Preserve containerd's trusted start-stdin options for the long-lived shim.
/// SandboxAPI CreateSandbox carries PodSandboxConfig instead of runtime options.
pub fn persist_startup_options(bundle: &Path, reader: impl Read) -> Result<()> {
    let data = read_startup_envelope(reader)?;
    let path = bundle.join(SHIM_RUNTIME_OPTIONS_FILE);
    if data.is_empty() || startup_options_payload(&data)?.is_none() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("remove stale shim startup options"),
        };
    }
    // Create a private file without following existing links, then atomically
    // replace the destination so a failed start cannot leave partial options.
    let temporary = bundle.join(format!(
        ".{SHIM_RUNTIME_OPTIONS_FILE}.{}",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .context("create shim startup options")?;
    let result = (|| {
        file.write_all(&data)?;
        file.sync_all()?;
        fs::rename(&temporary, &path).context("publish shim startup options")
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Load only a private, regular options file written by the shim start process.
pub fn load_startup_options(bundle: &Path) -> Result<Option<Vec<u8>>> {
    let path = bundle.join(SHIM_RUNTIME_OPTIONS_FILE);
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("open shim startup options"),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.uid() != nix::unistd::geteuid().as_raw()
    {
        return Err(anyhow!(
            "shim startup options must be a private, owned regular file"
        ));
    }
    let data = read_startup_envelope(file)?;
    startup_options_payload(&data)
}

/// Hypervisor plugins must be registered first, or hypervisor defaults are not applied.
///
/// Config override ordering(high to low):
/// 1. environment variable
/// 2. shimv2 create task option
/// 3. If above two are not set, then get default path from DEFAULT_RUNTIME_CONFIGURATIONS
/// in kata-containers/src/libs/kata-types/src/config/default.rs, in array order.
#[instrument]
pub fn load_config(an: &HashMap<String, String>, option: &Option<Vec<u8>>) -> Result<TomlConfig> {
    Ok(load_config_with_path(an, option)?.0)
}

/// Like `load_config`, also returning the configuration file that was loaded.
pub fn load_config_with_path(
    an: &HashMap<String, String>,
    option: &Option<Vec<u8>>,
) -> Result<(TomlConfig, PathBuf)> {
    let config_path = requested_config_path(option)?;
    load_config_from(an, &config_path)
}

/// Configuration file requested by the environment or the shimv2 options. An
/// empty path selects the first existing default configuration file.
fn requested_config_path(option: &Option<Vec<u8>>) -> Result<String> {
    const KATA_CONF_FILE: &str = "KATA_CONF_FILE";

    let config_path = if let Ok(path) = std::env::var(KATA_CONF_FILE) {
        if is_shipped_kata_config_path(&path) {
            path
        } else {
            return Err(anyhow!(
                "invalid KATA_CONF_FILE {:?}: only shipped Kata configuration files are accepted",
                path
            ));
        }
    } else if let Some(option) = option {
        // Parse the containerd runtime options protobuf message to extract the config path.
        // The options are passed as a serialized runtimeoptions.v1.Options protobuf message
        // from containerd's configuration (e.g., [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata.options]).
        match <protocols::runtimeoptions::Options as ProtobufMessage>::parse_from_bytes(option) {
            Ok(opts) => opts.config_path,
            Err(e) => {
                // Log the error but don't fail - fall back to default config paths
                let logger = slog::Logger::clone(&slog_scope::logger());
                slog::warn!(
                    logger,
                    "failed to parse containerd runtime options: {}, falling back to default config paths",
                    e
                );
                String::from("")
            }
        }
    } else {
        String::from("")
    };
    Ok(config_path)
}

/// Load `config_path`, or the first existing default configuration file when
/// it is empty, returning the configuration and the file that was loaded.
pub fn load_config_from<P: AsRef<Path> + std::fmt::Debug>(
    an: &HashMap<String, String>,
    config_path: P,
) -> Result<(TomlConfig, PathBuf)> {
    let annotation = Annotation::new(an.clone());
    // Clone a logger from global logger to ensure the logs in this function get flushed when drop
    let logger = slog::Logger::clone(&slog_scope::logger());

    info!(logger, "get config path {:?}", &config_path);
    let (mut toml_config, loaded_path) =
        TomlConfig::load_from_file(&config_path).context(format!(
            "load TOML config failed (tried {:?})",
            TomlConfig::get_default_config_file_list()
        ))?;
    annotation.update_config_by_annotation(&mut toml_config)?;
    update_agent_kernel_params(&mut toml_config)?;

    // validate configuration and return the error
    toml_config.validate()?;

    info!(logger, "get config content {:?}", &toml_config);
    Ok((toml_config, loaded_path))
}

fn is_shipped_kata_config_path(config_path: &str) -> bool {
    config_path_matches_defaults(config_path, TomlConfig::get_default_config_file_list())
}

fn config_path_matches_defaults(config_path: &str, default_config_paths: Vec<PathBuf>) -> bool {
    let Ok(resolved_config_path) = std::fs::canonicalize(config_path) else {
        return false;
    };

    default_config_paths
        .into_iter()
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .any(|path| path == resolved_config_path)
}

// this update the agent-specfic kernel parameters into hypervisor's bootinfo
// the agent inside the VM will read from file cmdline to get the params and function
fn update_agent_kernel_params(config: &mut TomlConfig) -> Result<()> {
    let mut params = vec![];
    if let Ok(kv) = config.get_agent_kernel_params() {
        for (k, v) in kv.into_iter() {
            if let Ok(s) = Param::new(k.as_str(), v.as_str()).to_string() {
                params.push(s);
            }
        }
        if let Some(h) = config.hypervisor.get_mut(&config.runtime.hypervisor_name) {
            h.boot_info.add_kernel_params(params);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn startup_envelope(config_path: &str) -> Vec<u8> {
        let mut options = protocols::runtimeoptions::Options::new();
        options.config_path = config_path.into();
        let mut envelope = protobuf::well_known_types::any::Any::new();
        envelope.type_url = "runtimeoptions.v1.Options".into();
        envelope.value = options.write_to_bytes().unwrap();
        envelope.write_to_bytes().unwrap()
    }

    fn command_bootstrap() -> protocols::bootstrap::BootstrapParams {
        protocols::bootstrap::BootstrapParams::parse_from_bytes(include_bytes!(
            "testdata/shim-start-bootstrap.pb"
        ))
        .unwrap()
    }

    #[test]
    fn startup_options_decode_actual_containerd_command_bootstrap() {
        let wire = include_bytes!("testdata/shim-start-bootstrap.pb");
        let bundle = tempfile::tempdir().unwrap();
        persist_startup_options(bundle.path(), wire.as_slice()).unwrap();
        let payload = load_startup_options(bundle.path()).unwrap().unwrap();
        let options = protocols::runtimeoptions::Options::parse_from_bytes(&payload).unwrap();
        assert_eq!(
            options.config_path,
            "/opt/kata/custom/configuration-qemu.toml"
        );
        let params = command_bootstrap();
        assert_eq!(params.instance_id, "bootstrap-fixture-sandbox");
        assert_eq!(params.namespace, "k8s.io");
        assert!(!params.containerd_binary.is_empty());
    }

    #[test]
    fn startup_options_bootstrap_no_options_and_unrelated_extensions_fall_back() {
        let mut params = command_bootstrap();
        params.extensions.clear();
        assert_eq!(
            startup_options_payload(&params.write_to_bytes().unwrap()).unwrap(),
            None
        );
        let mut extension = protocols::bootstrap::Extension::new();
        let mut value = protocols::bootstrap::BootstrapAny::new();
        value.type_url = "example.test.Unrelated".into();
        value.value = vec![0xff];
        extension.value = protobuf::MessageField::some(value);
        params.extensions.push(extension);
        assert_eq!(
            startup_options_payload(&params.write_to_bytes().unwrap()).unwrap(),
            None
        );
        params
            .extensions
            .push(command_bootstrap().extensions.remove(0));
        assert!(startup_options_payload(&params.write_to_bytes().unwrap())
            .unwrap()
            .is_some());
    }

    #[test]
    fn startup_options_bootstrap_reject_malformed_duplicates_and_missing_identity() {
        let mut params = command_bootstrap();
        params.extensions[0].value.as_mut().unwrap().value = vec![0xff];
        assert!(startup_options_payload(&params.write_to_bytes().unwrap()).is_err());
        let mut params = command_bootstrap();
        params.extensions.push(params.extensions[0].clone());
        assert!(startup_options_payload(&params.write_to_bytes().unwrap()).is_err());
        let mut params = command_bootstrap();
        params.instance_id = "runtimeoptions.v1.Options".into();
        // This is a legal containerd ID, not an Any type URL in bootstrap.
        assert!(startup_options_payload(&params.write_to_bytes().unwrap())
            .unwrap()
            .is_some());
        let mut params = command_bootstrap();
        params.namespace.clear();
        assert!(startup_options_payload(&params.write_to_bytes().unwrap()).is_err());
    }

    #[test]
    fn startup_options_decode_legacy_containerd_go_any_envelope() {
        // Generated by containerd v2.3.3's Go typeurl.MarshalAny/MarshalProto
        // using api/types/runtimeoptions/v1.Options for the legacy Any input.
        let wire = b"\x0a\x19runtimeoptions.v1.Options\x12\x2a\x12\x28/opt/kata/custom/configuration-qemu.toml";
        let bundle = tempfile::tempdir().unwrap();
        persist_startup_options(bundle.path(), wire.as_slice()).unwrap();
        let payload = load_startup_options(bundle.path()).unwrap().unwrap();
        assert_eq!(
            protocols::runtimeoptions::Options::parse_from_bytes(&payload)
                .unwrap()
                .config_path,
            "/opt/kata/custom/configuration-qemu.toml"
        );
    }

    #[test]
    fn startup_options_preserve_config_path_and_private_permissions() {
        let bundle = tempfile::tempdir().unwrap();
        let config_path = "/opt/kata/custom/configuration-qemu.toml";
        let envelope = startup_envelope(config_path);
        persist_startup_options(bundle.path(), envelope.as_slice()).unwrap();
        let payload = load_startup_options(bundle.path()).unwrap().unwrap();
        let options = protocols::runtimeoptions::Options::parse_from_bytes(&payload).unwrap();
        assert_eq!(options.config_path, config_path);
        let path = bundle.path().join(SHIM_RUNTIME_OPTIONS_FILE);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // Repeated starts replace the complete envelope instead of reusing it.
        persist_startup_options(
            bundle.path(),
            startup_envelope("/new/config.toml").as_slice(),
        )
        .unwrap();
        let payload = load_startup_options(bundle.path()).unwrap().unwrap();
        assert_eq!(
            protocols::runtimeoptions::Options::parse_from_bytes(&payload)
                .unwrap()
                .config_path,
            "/new/config.toml"
        );
    }

    #[test]
    fn startup_options_empty_input_and_default_any_preserve_fallback() {
        let bundle = tempfile::tempdir().unwrap();
        assert!(load_startup_options(bundle.path()).unwrap().is_none());
        for empty in [
            Vec::new(),
            protobuf::well_known_types::any::Any::new()
                .write_to_bytes()
                .unwrap(),
        ] {
            persist_startup_options(bundle.path(), startup_envelope("/stale.toml").as_slice())
                .unwrap();
            persist_startup_options(bundle.path(), empty.as_slice()).unwrap();
            assert!(load_startup_options(bundle.path()).unwrap().is_none());
        }
        // A typed but empty Options message is also valid (default config).
        persist_startup_options(bundle.path(), startup_envelope("").as_slice()).unwrap();
        assert_eq!(
            load_startup_options(bundle.path()).unwrap(),
            Some(Vec::new())
        );
    }

    #[test]
    fn startup_options_reject_malformed_and_unexpected_types() {
        let bundle = tempfile::tempdir().unwrap();
        let mut envelope = protobuf::well_known_types::any::Any::new();
        envelope.type_url = "other.v1.Options".into();
        let unexpected = envelope.write_to_bytes().unwrap();
        envelope.type_url = "runtimeoptions.v1.Options".into();
        envelope.value = vec![0xff];
        for data in [
            vec![0xff],
            unexpected,
            envelope.write_to_bytes().unwrap(),
            vec![0; MAX_STARTUP_OPTIONS_SIZE as usize + 1],
        ] {
            assert!(persist_startup_options(bundle.path(), data.as_slice()).is_err());
            assert!(load_startup_options(bundle.path()).unwrap().is_none());
        }
    }

    #[test]
    fn startup_options_reject_unsafe_files_and_revalidate_persisted_data() {
        let bundle = tempfile::tempdir().unwrap();
        let path = bundle.path().join(SHIM_RUNTIME_OPTIONS_FILE);
        persist_startup_options(bundle.path(), startup_envelope("/config.toml").as_slice())
            .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_startup_options(bundle.path()).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&path, [0xff]).unwrap();
        assert!(load_startup_options(bundle.path()).is_err());
        fs::remove_file(&path).unwrap();
        let target = bundle.path().join("target");
        fs::write(&target, startup_envelope("/config.toml")).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(load_startup_options(bundle.path()).is_err());
    }

    #[test]
    fn startup_options_do_not_remove_a_colliding_temporary_file() {
        let bundle = tempfile::tempdir().unwrap();
        let temporary = bundle.path().join(format!(
            ".{SHIM_RUNTIME_OPTIONS_FILE}.{}",
            std::process::id()
        ));
        fs::write(&temporary, b"existing file").unwrap();
        assert!(persist_startup_options(
            bundle.path(),
            startup_envelope("/config.toml").as_slice()
        )
        .is_err());
        assert_eq!(fs::read(&temporary).unwrap(), b"existing file");
        assert!(load_startup_options(bundle.path()).unwrap().is_none());
    }

    #[derive(Debug)]
    enum ConfigPathCase {
        Shipped,
        NonShipped,
        NonExistent,
        Empty,
    }

    #[rstest]
    #[case::shipped_config_is_accepted(ConfigPathCase::Shipped, true)]
    #[case::non_shipped_config_is_rejected(ConfigPathCase::NonShipped, false)]
    #[case::non_existent_path_is_rejected(ConfigPathCase::NonExistent, false)]
    #[case::empty_path_is_rejected(ConfigPathCase::Empty, false)]
    fn test_config_path_matches_defaults(
        #[case] path_case: ConfigPathCase,
        #[case] expected: bool,
    ) {
        let tmpdir = tempfile::tempdir().unwrap();
        let shipped_path = tmpdir.path().join("shipped.toml");
        let non_shipped_path = tmpdir.path().join("malicious.toml");
        std::fs::write(&shipped_path, b"[hypervisor.qemu]\n").unwrap();
        std::fs::write(&non_shipped_path, b"[hypervisor.qemu]\n").unwrap();

        // Only the shipped path is treated as a default config location.
        let default_config_paths = vec![shipped_path.clone()];

        let config_path = match path_case {
            ConfigPathCase::Shipped => shipped_path.to_string_lossy().to_string(),
            ConfigPathCase::NonShipped => non_shipped_path.to_string_lossy().to_string(),
            ConfigPathCase::NonExistent => tmpdir
                .path()
                .join("nonexistent.toml")
                .to_string_lossy()
                .to_string(),
            ConfigPathCase::Empty => String::new(),
        };

        assert_eq!(
            config_path_matches_defaults(&config_path, default_config_paths),
            expected,
            "case {:?}: unexpected result for path {:?}",
            path_case,
            config_path,
        );
    }
}
