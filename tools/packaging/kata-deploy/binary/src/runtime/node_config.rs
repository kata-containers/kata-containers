// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// A node's containerd settings can sit in an import or a drop-in as easily as
// in the main file, so finding them means reading all of them.

use crate::config::ContainerdPaths;
use crate::runtime::containerd;
use crate::utils::toml as toml_utils;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Override {
    pub path: String,
    pub file: PathBuf,
    pub node_value: String,
    pub our_value: String,
}

impl fmt::Display for Override {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is {} in {}, and this install writes {}",
            self.path,
            self.node_value,
            self.file.display(),
            self.our_value
        )
    }
}

/// Excluded, or a redeploy would fail against its own last run. Only ours: a
/// sibling shares this containerd and its snapshotter state, so it binds us.
fn our_own_files(paths: &ContainerdPaths, multi_install_suffix: Option<&str>) -> Vec<PathBuf> {
    if !paths.use_drop_in {
        return Vec::new();
    }

    let mut ours = vec![PathBuf::from(&paths.drop_in_file)];

    if let Ok((user_drop_in, _)) =
        containerd::get_user_containerd_drop_in_output_path(paths, multi_install_suffix)
    {
        ours.push(user_drop_in);
    }

    ours
}

/// `imports` globs are only ever in the file name.
fn expand_import(pattern: &str) -> Vec<PathBuf> {
    let pattern = Path::new(pattern.trim().trim_matches(['"', '\'']));

    let Some(name) = pattern.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    if !name.contains('*') {
        return vec![pattern.to_path_buf()];
    }

    let Some(parent) = pattern.parent() else {
        return Vec::new();
    };
    let (prefix, suffix) = name.split_once('*').unwrap_or((name, ""));
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };

    let mut matched: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.len() >= prefix.len() + suffix.len()
                        && name.starts_with(prefix)
                        && name.ends_with(suffix)
                })
        })
        .collect();

    // containerd reads a directory in name order, and later files win.
    matched.sort();
    matched
}

/// Earliest first, as containerd reads them.
pub fn sources(paths: &ContainerdPaths, multi_install_suffix: Option<&str>) -> Vec<PathBuf> {
    // We edit the main file in place without drop-ins, so only the backup taken
    // on first install still holds what the node had.
    let main = if paths.use_drop_in {
        PathBuf::from(&paths.config_file)
    } else {
        PathBuf::from(&paths.backup_file)
    };

    let mut found = vec![main.clone()];

    for pattern in toml_utils::get_toml_array(&main, ".imports").unwrap_or_default() {
        found.extend(expand_import(&pattern));
    }

    // An auto-loaded directory declares no import to find it by. Guarded, or
    // without drop-ins this reads back the main file we just edited.
    if paths.use_drop_in && paths.imports_file.is_none() {
        if let Some(dir) = Path::new(&paths.drop_in_file).parent() {
            found.extend(expand_import(&dir.join("*.toml").to_string_lossy()));
        }
    }

    let ours = our_own_files(paths, multi_install_suffix);

    let mut seen = HashSet::new();
    found.retain(|path| path.is_file() && !ours.contains(path) && seen.insert(path.clone()));

    found
}

/// Values read back have lost their quoting, so a literal of ours has to too.
fn as_read(literal: &str) -> String {
    let literal = literal.trim();

    match literal
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    {
        Some(items) => {
            let items: Vec<&str> = items
                .split(',')
                .map(|item| item.trim().trim_matches(['"', '\'']))
                .filter(|item| !item.is_empty())
                .collect();
            format!("[{}]", items.join(", "))
        }
        None => literal.trim_matches(['"', '\'']).to_string(),
    }
}

/// Searched in reverse, because containerd's merge gives the last file the word.
pub fn states(sources: &[PathBuf], path: &str) -> Option<(PathBuf, String)> {
    sources.iter().rev().find_map(|file| {
        toml_utils::get_toml_value(file, path)
            .ok()
            .map(|value| (file.clone(), value))
    })
}

pub fn table_stated(sources: &[PathBuf], table: &str) -> bool {
    sources
        .iter()
        .any(|file| toml_utils::get_toml_table_keys(file, table).is_ok_and(|keys| !keys.is_empty()))
}

/// `ours` holds TOML literals, as they would be written.
pub fn overrides<P, V>(sources: &[PathBuf], ours: &[(P, V)]) -> Vec<Override>
where
    P: AsRef<str>,
    V: AsRef<str>,
{
    ours.iter()
        .filter_map(|(path, our_value)| {
            let path = path.as_ref();
            let our_value = as_read(our_value.as_ref());
            let (file, node_value) = states(sources, path)?;

            (node_value != our_value).then(|| Override {
                path: path.to_string(),
                file,
                node_value,
                our_value,
            })
        })
        .collect()
}

/// Taking these is fine; taking them silently is not.
pub fn warn_about_overrides<P, V>(sources: &[PathBuf], ours: &[(P, V)])
where
    P: AsRef<str>,
    V: AsRef<str>,
{
    for taken in overrides(sources, ours) {
        log::warn!("containerd config taken over by this install: {taken}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::create_dir_all(dir).unwrap();
        fs::write(&path, body).unwrap();
        path
    }

    fn paths(config_file: &Path, drop_in_file: &Path, imports: bool) -> ContainerdPaths {
        ContainerdPaths {
            config_file: config_file.to_string_lossy().to_string(),
            backup_file: format!("{}.bak", config_file.display()),
            imports_file: imports.then(|| config_file.to_string_lossy().to_string()),
            drop_in_file: drop_in_file.to_string_lossy().to_string(),
            use_drop_in: true,
            plugin_id: None,
        }
    }

    #[test]
    fn a_sibling_drop_in_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let conf_d = dir.path().join("conf.d");
        let config = write(dir.path(), "config.toml", "version = 3\n");
        write(&conf_d, "00-operator.toml", "[debug]\nlevel = 'warn'\n");
        let drop_in = conf_d.join("kata-deploy.toml");

        let sources = sources(&paths(&config, &drop_in, false), None);

        assert_eq!(
            states(&sources, ".debug.level").map(|(_, value)| value),
            Some("warn".to_string())
        );
    }

    #[test]
    fn our_own_files_are_not_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let conf_d = dir.path().join("conf.d");
        let config = write(dir.path(), "config.toml", "version = 3\n");
        write(&conf_d, "kata-deploy.toml", "[debug]\nlevel = 'debug'\n");
        write(
            &conf_d,
            "zz-kata-deploy-user.toml",
            "[debug]\nlevel = 'trace'\n",
        );
        let drop_in = conf_d.join("kata-deploy.toml");

        let sources = sources(&paths(&config, &drop_in, false), None);

        assert!(states(&sources, ".debug.level").is_none(), "{sources:?}");
    }

    #[test]
    fn another_installations_drop_in_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let conf_d = dir.path().join("conf.d");
        let config = write(dir.path(), "config.toml", "version = 3\n");
        write(&conf_d, "kata-deploy.toml", "[debug]\nlevel = 'warn'\n");
        write(
            &conf_d,
            "zz-kata-deploy-user.toml",
            "[debug]\nlevel = 'trace'\n",
        );

        let drop_in = conf_d.join("kata-deploy-beta.toml");
        let sources = sources(&paths(&config, &drop_in, false), Some("beta"));

        let (file, level) = states(&sources, ".debug.level").unwrap();

        assert_eq!(level, "trace", "{sources:?}");
        assert!(file.ends_with("zz-kata-deploy-user.toml"), "{file:?}");
    }

    #[test]
    fn imports_are_expanded_and_missing_ones_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let conf_d = dir.path().join("conf.d");
        write(&conf_d, "10-erofs.toml", "[debug]\nlevel = 'info'\n");
        let config = write(
            dir.path(),
            "config.toml",
            &format!(
                "version = 3\nimports = ['{}/*.toml', '/nonexistent/x.toml']\n",
                conf_d.display()
            ),
        );
        let drop_in = dir.path().join("kata-deploy.toml");

        let sources = sources(&paths(&config, &drop_in, true), None);

        assert_eq!(sources.len(), 2, "{sources:?}");
        assert!(sources[1].ends_with("10-erofs.toml"), "{sources:?}");
    }

    #[test]
    fn the_last_file_to_state_a_setting_wins() {
        let dir = tempfile::tempdir().unwrap();
        let conf_d = dir.path().join("conf.d");
        let config = write(dir.path(), "config.toml", "[debug]\nlevel = 'warn'\n");
        write(&conf_d, "90-late.toml", "[debug]\nlevel = 'trace'\n");
        let drop_in = conf_d.join("kata-deploy.toml");

        let sources = sources(&paths(&config, &drop_in, false), None);
        let (file, value) = states(&sources, ".debug.level").unwrap();

        assert_eq!(value, "trace");
        assert!(file.ends_with("90-late.toml"));
    }

    #[rstest]
    #[case("\"10G\"", "10G", false)]
    #[case("true", "true", false)]
    #[case("0", "0", false)]
    #[case("[\"erofs\",\"walking\"]", "[erofs, walking]", false)]
    #[case("\"10G\"", "6G", true)]
    #[case("[\"erofs\",\"walking\"]", "[walking]", true)]
    fn a_literal_is_compared_as_it_reads_back(
        #[case] ours: &str,
        #[case] node: &str,
        #[case] conflicts: bool,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let body = if node.starts_with('[') {
            let items: Vec<String> = node
                .trim_matches(['[', ']'])
                .split(',')
                .map(|item| format!("'{}'", item.trim()))
                .collect();
            format!("[table]\nkey = [{}]\n", items.join(", "))
        } else if node == "true" || node == "false" || node.parse::<i64>().is_ok() {
            format!("[table]\nkey = {node}\n")
        } else {
            format!("[table]\nkey = '{node}'\n")
        };
        let config = write(dir.path(), "config.toml", &body);
        let drop_in = dir.path().join("kata-deploy.toml");

        let sources = sources(&paths(&config, &drop_in, true), None);
        let found = overrides(&sources, &[(".table.key", ours.to_string())]);

        assert_eq!(!found.is_empty(), conflicts, "{:?}", found.len());
    }

    #[test]
    fn a_setting_the_node_leaves_alone_is_not_an_override() {
        let dir = tempfile::tempdir().unwrap();
        let config = write(dir.path(), "config.toml", "version = 3\n");
        let drop_in = dir.path().join("kata-deploy.toml");

        let sources = sources(&paths(&config, &drop_in, true), None);

        assert!(overrides(&sources, &[(".debug.level", "\"debug\"".to_string())]).is_empty());
    }

    #[test]
    fn whole_file_mode_reads_the_backup() {
        let dir = tempfile::tempdir().unwrap();
        let config = write(dir.path(), "config.toml", "[debug]\nlevel = 'debug'\n");
        write(dir.path(), "config.toml.bak", "[debug]\nlevel = 'warn'\n");

        let mut paths = paths(&config, &config, false);
        paths.use_drop_in = false;

        let sources = sources(&paths, None);

        assert_eq!(
            states(&sources, ".debug.level").map(|(_, value)| value),
            Some("warn".to_string())
        );
    }

    #[test]
    fn a_table_counts_as_stated_even_when_no_key_of_ours_is() {
        let dir = tempfile::tempdir().unwrap();
        let config = write(
            dir.path(),
            "config.toml",
            "[plugins.'io.containerd.snapshotter.v1.erofs']\nroot_path = '/x'\n",
        );
        let drop_in = dir.path().join("kata-deploy.toml");

        let sources = sources(&paths(&config, &drop_in, true), None);

        assert!(table_stated(
            &sources,
            ".plugins.\"io.containerd.snapshotter.v1.erofs\""
        ));
        assert!(!table_stated(
            &sources,
            ".plugins.\"io.containerd.differ.v1.erofs\""
        ));
    }
}
