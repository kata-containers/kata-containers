// Copyright Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

pub use drop_in_directory_handling::load;

mod toml_tree_ops {
    // The following pair of functions implement toml::Value tree merging, with
    // the second argument being merged into the first one and consumed in the
    // process.  The toml parser crate in use here doesn't support parsing into
    // a pre-existing (possibly pre-filled) TomlConfig instance but can parse
    // into a toml::Value tree so we use that instead.  All files (base and
    // drop-ins) are initially parsed into toml::Value trees which are
    // subsequently merged.  Only when the fully merged tree is computed it is
    // converted to a TomlConfig instance.

    fn merge_tables(base_table: &mut toml::value::Table, dropin_table: toml::value::Table) {
        for (key, val) in dropin_table.into_iter() {
            match base_table.get_mut(&key) {
                Some(base_val) => merge(base_val, val),
                None => {
                    base_table.insert(key, val);
                }
            }
        }
    }

    pub fn merge(base: &mut toml::Value, dropin: toml::Value) {
        match dropin {
            toml::Value::Table(dropin_table) => {
                if let toml::Value::Table(base_table) = base {
                    merge_tables(base_table, dropin_table);
                } else {
                    *base = toml::Value::Table(dropin_table);
                }
            }

            _ => *base = dropin,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // Mock config structure to stand in for TomlConfig for low-level
        // toml::Value trees merging.
        #[derive(Deserialize, Debug, Default, PartialEq)]
        struct SubConfig {
            #[serde(default)]
            another_string: String,
            #[serde(default)]
            yet_another_number: i32,
            #[serde(default)]
            sub_array: Vec<i32>,
        }

        #[derive(Deserialize, Debug, Default, PartialEq)]
        struct Config {
            #[serde(default)]
            number: i32,
            #[serde(default)]
            string: String,
            #[serde(default)]
            another_number: u8,
            #[serde(default)]
            array: Vec<i32>,

            #[serde(default)]
            sub: SubConfig,
        }

        #[test]
        fn dropin_does_not_interfere_with_base() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = 42
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                string = "foo"
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 42,
                    string: "foo".into(),
                    sub: Default::default(),
                    ..Default::default()
                })
            );
        }

        #[test]
        fn dropin_overrides_base() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = 42
                [sub]
                another_string = "foo"
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                number = 43
                [sub]
                another_string = "bar"
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 43,
                    sub: SubConfig {
                        another_string: "bar".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            );
        }

        #[test]
        fn dropin_extends_base() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = 42
                [sub]
                another_string = "foo"
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                string = "hello"
                [sub]
                yet_another_number = 13
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 42,
                    string: "hello".into(),
                    sub: SubConfig {
                        another_string: "foo".into(),
                        yet_another_number: 13,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            );
        }

        // Drop-ins can change the type of a value.  This might look weird but at
        // this level we have no idea about semantics so we just do what the
        // .toml's tell us.  The final type check is only performed by try_into().
        // Also, we don't necessarily test this because it's a desired feature.
        // It's just something that seems to follow from the way Value tree
        // merging is implemented so why not acknowledge and verify it.
        #[test]
        fn dropin_overrides_base_type() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = "foo"
                [sub]
                another_string = 42
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                number = 42
                [sub]
                another_string = "foo"
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 42,
                    sub: SubConfig {
                        another_string: "foo".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            );
        }
    }
}

mod drop_in_directory_handling {
    use crate::config::TomlConfig;
    use std::fs;
    use std::io::{self, Result};
    use std::path::{Path, PathBuf};

    fn get_dropin_dir_path(base_cfg_file_path: &Path) -> Result<PathBuf> {
        let mut dropin_dir = base_cfg_file_path.to_path_buf();
        if !dropin_dir.pop() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "base cfg file path too short",
            ));
        }
        dropin_dir.push("config.d");
        Ok(dropin_dir)
    }

    fn update_from_dropin(base_config: &mut toml::Value, dropin_file: &fs::DirEntry) -> Result<()> {
        if !dropin_file.file_type()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "drop-in cfg file can only be a regular file or a symlink",
            ));
        }
        let dropin_contents = fs::read_to_string(dropin_file.path())?;
        let dropin_config: toml::Value = toml::from_str(&dropin_contents)?;
        super::toml_tree_ops::merge(base_config, dropin_config);
        Ok(())
    }

    fn update_from_dropins(base_config: &mut toml::Value, dropin_dir: &Path) -> Result<()> {
        let dropin_files_iter = match fs::read_dir(dropin_dir) {
            Ok(iter) => iter,
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    return Ok(());
                } else {
                    return Err(err);
                }
            }
        };

        let mut dropin_files = dropin_files_iter.collect::<Result<Vec<_>>>()?;
        dropin_files.sort_by_key(|direntry| direntry.file_name());
        for dropin_file in &dropin_files {
            update_from_dropin(base_config, dropin_file)?;
        }
        Ok(())
    }

    pub fn load(base_cfg_file_path: &Path) -> Result<TomlConfig> {
        let base_toml_str = fs::read_to_string(base_cfg_file_path)?;
        let mut base_config: toml::Value = toml::from_str(&base_toml_str)?;
        let dropin_dir = get_dropin_dir_path(base_cfg_file_path)?;

        update_from_dropins(&mut base_config, &dropin_dir)?;

        let config: TomlConfig = base_config.try_into()?;
        Ok(config)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Write;

        const BASE_CONFIG_DATA: &str = r#"
            [hypervisor.qemu]
            path = "/usr/bin/qemu-kvm"
            default_bridges = 3
            [runtime]
            enable_debug = true
            internetworking_model="tcfilter"
        "#;

        fn check_base_config(config: &TomlConfig) {
            assert_eq!(
                config.hypervisor["qemu"].path,
                "/usr/bin/qemu-kvm".to_string()
            );
            assert_eq!(config.hypervisor["qemu"].device_info.default_bridges, 3);
            assert!(config.runtime.debug);
            assert_eq!(config.runtime.internetworking_model, "tcfilter".to_string());
        }

        fn create_file(path: &Path, contents: &[u8]) -> Result<()> {
            fs::File::create(path)?.write_all(contents)
        }

        #[test]
        fn test_no_dropins_dir() {
            let tmpdir = tempfile::tempdir().unwrap();

            let config_path = tmpdir.path().join("runtime.toml");
            create_file(&config_path, BASE_CONFIG_DATA.as_bytes()).unwrap();

            let config = load(&config_path).unwrap();
            check_base_config(&config);
        }

        #[test]
        fn test_no_dropins() {
            let tmpdir = tempfile::tempdir().unwrap();

            let config_path = tmpdir.path().join("runtime.toml");
            create_file(&config_path, BASE_CONFIG_DATA.as_bytes()).unwrap();

            let dropin_dir = tmpdir.path().join("config.d");
            fs::create_dir(dropin_dir).unwrap();

            let config = load(&config_path).unwrap();
            check_base_config(&config);
        }

        #[test]
        fn test_dropins() {
            let tmpdir = tempfile::tempdir().unwrap();

            let dropin_data = r#"
                [hypervisor.qemu]
                default_vcpus = 2
                default_bridges = 4
                shared_fs = "virtio-fs"
                [runtime]
                sandbox_cgroup_only=true
                internetworking_model="macvtap"
                vfio_mode="guest-kernel"
            "#;

            let dropin_override_data = r#"
                [hypervisor.qemu]
                shared_fs = "virtio-9p"
                [runtime]
                vfio_mode="vfio"
            "#;

            let config_path = tmpdir.path().join("runtime.toml");
            create_file(&config_path, BASE_CONFIG_DATA.as_bytes()).unwrap();

            let dropin_dir = tmpdir.path().join("config.d");
            fs::create_dir(&dropin_dir).unwrap();

            let dropin_path = dropin_dir.join("10-base");
            create_file(&dropin_path, dropin_data.as_bytes()).unwrap();

            let dropin_override_path = dropin_dir.join("20-override");
            create_file(&dropin_override_path, dropin_override_data.as_bytes()).unwrap();

            let config = load(&config_path).unwrap();
            assert_eq!(
                config.hypervisor["qemu"].path,
                "/usr/bin/qemu-kvm".to_string()
            );
            assert_eq!(config.hypervisor["qemu"].cpu_info.default_vcpus, 2);
            assert_eq!(config.hypervisor["qemu"].device_info.default_bridges, 4);
            assert_eq!(
                config.hypervisor["qemu"].shared_fs.shared_fs.as_deref(),
                Some("virtio-9p")
            );
            assert!(config.runtime.debug);
            assert!(config.runtime.sandbox_cgroup_only);
            assert_eq!(config.runtime.internetworking_model, "macvtap".to_string());
            assert_eq!(config.runtime.vfio_mode, "vfio".to_string());
        }
    }
}
