// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oci_spec::image::ImageConfiguration;
use oci_spec::runtime::{Mount, Process, Spec};

pub const BUNDLE_CONFIG: &str = "config.json";
pub const BUNDLE_ROOTFS: &str = "rootfs";
pub const BUNDLE_HOSTNAME: &str = "image-rs";

const ANNOTATION_OS: &str = "org.opencontainers.image.os";
const ANNOTATION_OS_VERSION: &str = "org.opencontainers.image.os.version";
const ANNOTATION_OS_FEATURES: &str = "org.opencontainers.image.os.features";
const ANNOTATION_ARCH: &str = "org.opencontainers.image.architecture";
const ANNOTATION_VARIANT: &str = "org.opencontainers.image.variant";
const ANNOTATION_AUTHOR: &str = "org.opencontainers.image.author";
const ANNOTATION_CREATED: &str = "org.opencontainers.image.created";
const ANNOTATION_STOP_SIGNAL: &str = "org.opencontainers.image.stopSignal";
const ANNOTATION_EXPOSED_PORTS: &str = "org.opencontainers.image.exposedPorts";

/// Convert an `application/vnd.oci.image.config.v1+json` object into an OCI runtime configuration
/// blob and write to `config.json`.
///
/// [OCI Image Spec: Conversion to OCI Runtime Configuration](https://github.com/opencontainers/image-spec/blob/main/conversion.md)
/// states:
///
/// The "default generated runtime configuration" MAY be overridden or combined with externally
/// provided inputs from the caller. In addition, a converter MAY have its own
/// implementation-defined defaults and extensions which MAY be combined with the "default generated
/// runtime configuration".
pub fn create_runtime_config(
    image_config: &ImageConfiguration,
    bundle_path: &Path,
) -> Result<PathBuf> {
    let mut spec = Spec::default();
    let mut annotations: HashMap<String, String> = HashMap::new();
    let mut labels = HashMap::new();

    // Update the default hostname
    spec.set_hostname(Some(BUNDLE_HOSTNAME.to_string()));

    if let Some(config) = image_config.config() {
        let mut process = Process::default();

        // Verbatim Fields:
        //
        // A compliant configuration converter MUST extract the following fields verbatim to the
        // corresponding field in the generated runtime configuration:
        // - WorkingDir
        // - Env
        // - EntryPoint
        // - Cmd
        if let Some(working_dir) = config.working_dir() {
            process.set_cwd(PathBuf::from(working_dir));
        }
        // The converter MAY add additional entries to process.env but it SHOULD NOT add entries
        // that have variable names present in Config.Env.
        if let Some(env) = config.env() {
            process.set_env(Some(env.to_vec()));
        }
        // If both Config.Entrypoint and Config.Cmd are specified, the converter MUST append the
        // value of Config.Cmd to the value of Config.Entrypoint and set process.args to that
        // combined value.
        let mut args: Vec<String> = vec![];
        if let Some(entrypoint) = config.entrypoint() {
            args.extend(entrypoint.clone());
        }
        if let Some(cmd) = config.cmd() {
            args.extend(cmd.clone());
        }
        if !args.is_empty() {
            process.set_args(Some(args));
        }
        spec.set_process(Some(process));

        // Annotation Fields:
        //
        // These fields all affect the annotations of the runtime configuration, and are thus
        // subject to precedence: if there is a conflict (same key but different value) between an
        // implicit annotation and an explicitly specified annotation in Config.Labels, the value
        // specified in Config.Labels MUST take precedence.
        // - os: org.opencontainers.image.os
        // - os.version: org.opencontainers.image.os.version
        // - os.features: org.opencontainers.image.os.features
        // - architecture: org.opencontainers.image.architecture
        // - variant: org.opencontainers.image.variant
        // - author: org.opencontainers.image.author
        // - created: org.opencontainers.image.created
        // - Config.StopSignal: org.opencontainers.image.stopSignal
        // - Config.Labels
        if let Some(labels2) = config.labels() {
            annotations.extend(labels2.clone());
            labels = labels2.clone();
        }
        if !labels.contains_key(ANNOTATION_STOP_SIGNAL) {
            if let Some(stop_signal) = config.stop_signal() {
                annotations.insert(ANNOTATION_STOP_SIGNAL.to_string(), stop_signal.to_string());
            }
        }

        // Parsed Fields:
        //
        // Certain image configuration fields have a counterpart that must first be translated.
        // A compliant configuration converter SHOULD parse all of these fields and set the
        // corresponding fields in the generated runtime configuration:
        // - User
        // TODO: parse image config user info and extract uid from rootfs passwd file
        // github issue: https://github.com/confidential-containers/image-rs/issues/8

        // Optional Fields:
        //
        // Certain image configuration fields are not applicable to all conversion use cases, and
        // thus are optional for configuration converters to implement. A compliant configuration
        // converter SHOULD provide a way for users to extract these fields into the generated
        // runtime configuration:
        // - ExposedPorts
        // - Volumes

        // The runtime configuration does not have a corresponding field for this image field.
        // However, converters SHOULD set the org.opencontainers.image.exposedPorts annotation.
        if let Some(exposed_ports) = config.exposed_ports() {
            annotations.insert(
                ANNOTATION_EXPOSED_PORTS.to_string(),
                exposed_ports.join(","),
            );
        }

        // Implementations SHOULD provide mounts for these locations such that application data is
        // not written to the container's root filesystem. If a converter implements conversion for
        // this field using mountpoints, it SHOULD set the destination of the mountpoint to the
        // value specified in Config.Volumes. An implementation MAY seed the contents of the mount
        // with data in the image at the same location. If a new image is created from a container
        // based on the image described by this configuration, data in these paths SHOULD NOT be
        // included in the new image. The other mounts fields are platform and context dependent,
        // and thus are implementation-defined.
        //
        // Note that the implementation of Config.Volumes need not use mountpoints, as it is
        // effectively a mask of the filesystem.
        if let Some(volumes) = config.volumes() {
            let mut mounts: Vec<Mount> = vec![];
            for v in volumes.iter() {
                let mut m = Mount::default();
                m.set_destination(PathBuf::from(v))
                    .set_typ("tmpfs".to_string().into())
                    .set_source(None)
                    .set_options(Some(vec![
                        "nosuid".into(),
                        "noexec".into(),
                        "nodev".into(),
                        "relatime".into(),
                        "rw".into(),
                    ]));
                mounts.push(m.clone());
            }
            if !mounts.is_empty() {
                if let Some(default_mounts) = spec.mounts() {
                    mounts.extend(default_mounts.clone());
                }
                spec.set_mounts(Some(mounts));
            }
        }
    }

    if !labels.contains_key(ANNOTATION_OS) {
        annotations.insert(ANNOTATION_OS.to_string(), image_config.os().to_string());
    }
    if !labels.contains_key(ANNOTATION_OS_VERSION) {
        if let Some(version) = image_config.os_version() {
            annotations.insert(ANNOTATION_OS_VERSION.to_string(), version.to_string());
        }
    }
    if !labels.contains_key(ANNOTATION_OS_FEATURES) {
        if let Some(features) = image_config.os_features() {
            if let Ok(v) = serde_json::to_string(features) {
                annotations.insert(ANNOTATION_OS_FEATURES.to_string(), v);
            }
        }
    }
    if !labels.contains_key(ANNOTATION_ARCH) {
        annotations.insert(
            ANNOTATION_ARCH.to_string(),
            image_config.architecture().to_string(),
        );
    }
    if !labels.contains_key(ANNOTATION_VARIANT) {
        if let Some(variant) = image_config.variant() {
            annotations.insert(ANNOTATION_VARIANT.to_string(), variant.to_string());
        }
    }
    if !labels.contains_key(ANNOTATION_AUTHOR) {
        if let Some(author) = image_config.author() {
            annotations.insert(ANNOTATION_AUTHOR.to_string(), author.to_string());
        }
    }
    if !labels.contains_key(ANNOTATION_CREATED) {
        if let Some(created) = image_config.created() {
            annotations.insert(ANNOTATION_CREATED.to_string(), created.to_string());
        }
    }

    spec.set_annotations(Some(annotations));
    let bundle_config = bundle_path.join(BUNDLE_CONFIG);

    if bundle_config.exists() {
        bail!("OCI config file already exists: {:?}", bundle_config);
    }

    spec.save(&bundle_config)?;

    Ok(bundle_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;

    #[test]
    fn test_bundle_create_config() {
        let image_config = ImageConfiguration::default();

        let tempdir = tempfile::tempdir().unwrap();
        let filename = tempdir.path().join(BUNDLE_CONFIG);

        assert!(!filename.exists());

        assert!(create_runtime_config(&image_config, tempdir.path()).is_ok());
        assert!(filename.exists());

        assert!(create_runtime_config(&image_config, tempdir.path()).is_err());
        assert!(filename.exists());
    }
}
