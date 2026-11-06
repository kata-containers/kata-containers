// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod sandbox_persist;
use anyhow::{Context, Result};
use kata_types::config::KATA_PATH;
use serde::de;
use std::{
    fs::{DirBuilder, File},
    io::{BufReader, Write},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

pub const PERSIST_FILE: &str = "state.json";
use kata_sys_util::validate::verify_id;
use safe_path::scoped_join;

/// state.json lives in the root-owned /run/kata/<sid> for every sandbox,
/// rootless and jailed ones included: root recovery trusts it, so nothing the
/// VMM user can write may hold or replace it.
pub fn state_dir(sid: &str) -> Result<PathBuf> {
    state_dir_in(Path::new(KATA_PATH), sid)
}

pub fn state_path(sid: &str) -> Result<PathBuf> {
    Ok(state_dir(sid)?.join(PERSIST_FILE))
}

pub fn to_disk<T: serde::Serialize>(value: &T, sid: &str) -> Result<()> {
    to_disk_in(Path::new(KATA_PATH), value, sid)
}

pub fn from_disk<T>(sid: &str) -> Result<T>
where
    T: de::DeserializeOwned,
{
    from_disk_in(Path::new(KATA_PATH), sid)
}

pub fn remove_from_disk(sid: &str) -> Result<()> {
    remove_from_disk_in(Path::new(KATA_PATH), sid)
}

/// The last step of a sandbox cleanup: remove its directory, the state with it.
pub fn remove_from_disk_in(root: &Path, sid: &str) -> Result<()> {
    verify_id(sid).context("failed to verify sid")?;
    if !root.join(sid).exists() {
        return Ok(());
    }
    let dir = state_dir_in(root, sid)?;
    match std::fs::remove_dir_all(&dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("failed to remove {}", dir.display()))
        }
        _ => Ok(()),
    }
}

pub fn state_dir_in(root: &Path, sid: &str) -> Result<PathBuf> {
    verify_id(sid).context("failed to verify sid")?;
    Ok(scoped_join(root, sid)?)
}

/// The state is rewritten as the sandbox makes progress, so a crash can land
/// in the middle of a write. Replace it with a rename to leave either the
/// previous state or the new one, never a truncated file.
pub fn to_disk_in<T: serde::Serialize>(root: &Path, value: &T, sid: &str) -> Result<()> {
    verify_id(sid).context("failed to verify sid")?;
    // The state is saved before the shim management server binds its socket
    // in this directory, possibly before the root exists at all, and
    // scoped_join() only resolves paths that exist.
    std::fs::create_dir_all(root)
        .with_context(|| format!("failed to create {}", root.display()))?;
    let unresolved = root.join(sid);
    DirBuilder::new()
        .recursive(true)
        .mode(0o750)
        .create(&unresolved)
        .with_context(|| format!("failed to create {}", unresolved.display()))?;
    let dir = state_dir_in(root, sid)?;

    let mut tmp = tempfile::Builder::new()
        .prefix(PERSIST_FILE)
        .tempfile_in(&dir)
        .context("failed to create the temporary state file")?;
    serde_json::to_writer_pretty(&mut tmp, value).context("failed to serialize the state")?;
    tmp.flush().context("failed to write the state")?;
    tmp.persist(dir.join(PERSIST_FILE))
        .context("failed to replace the state file")?;
    Ok(())
}

pub fn from_disk_in<T>(root: &Path, sid: &str) -> Result<T>
where
    T: de::DeserializeOwned,
{
    let path = state_dir_in(root, sid)?.join(PERSIST_FILE);
    let file = File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
    serde_json::from_reader(BufReader::new(file))
        .with_context(|| format!("failed to parse {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};
    use std::os::unix::fs::PermissionsExt;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Kata {
        name: String,
        key: u8,
    }

    fn kata(key: u8) -> Kata {
        Kata {
            name: "kata".to_string(),
            key,
        }
    }

    #[rstest]
    #[case::dot_dot("..3")]
    #[case::escapes_root("../../../3")]
    #[case::nested("a/b/c")]
    #[case::hidden(".#cdscd.")]
    fn invalid_sid_is_rejected(#[case] sid: &str) {
        let root = tempfile::tempdir().unwrap();
        assert!(to_disk_in(root.path(), &kata(1), sid).is_err());
        assert!(from_disk_in::<Kata>(root.path(), sid).is_err());
    }

    #[test]
    fn state_round_trips_and_creates_the_sandbox_dir() {
        let root = tempfile::tempdir().unwrap();
        to_disk_in(root.path(), &kata(1), "aadede").unwrap();

        let mode = std::fs::metadata(root.path().join("aadede"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o007, 0, "others must not reach the state");
        assert_eq!(
            from_disk_in::<Kata>(root.path(), "aadede").unwrap(),
            kata(1)
        );
    }

    #[test]
    fn the_first_save_on_a_fresh_host_creates_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("run").join("kata");
        to_disk_in(&root, &kata(1), "sid").unwrap();
        assert_eq!(from_disk_in::<Kata>(&root, "sid").unwrap(), kata(1));
    }

    #[rstest]
    #[case::sandbox_dir_removed(true)]
    #[case::root_missing(false)]
    fn removing_a_state_whose_directory_is_gone_succeeds(#[case] root_exists: bool) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("kata");
        if root_exists {
            std::fs::create_dir(&root).unwrap();
        }
        remove_from_disk_in(&root, "sid").unwrap();
    }

    #[test]
    fn saving_again_replaces_the_state_and_leaves_no_temporary_file() {
        let root = tempfile::tempdir().unwrap();
        to_disk_in(root.path(), &kata(1), "sid").unwrap();
        to_disk_in(root.path(), &kata(2), "sid").unwrap();

        assert_eq!(from_disk_in::<Kata>(root.path(), "sid").unwrap(), kata(2));
        let files: Vec<_> = std::fs::read_dir(root.path().join("sid"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(files, vec![PERSIST_FILE]);
    }

    #[test]
    fn a_failed_save_keeps_the_previous_state() {
        struct Broken;
        impl Serialize for Broken {
            fn serialize<S: serde::Serializer>(
                &self,
                _: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("boom"))
            }
        }

        let root = tempfile::tempdir().unwrap();
        to_disk_in(root.path(), &kata(1), "sid").unwrap();
        assert!(to_disk_in(root.path(), &Broken, "sid").is_err());
        assert_eq!(from_disk_in::<Kata>(root.path(), "sid").unwrap(), kata(1));
    }

    #[test]
    fn removing_the_state_twice_succeeds() {
        let root = tempfile::tempdir().unwrap();
        to_disk_in(root.path(), &kata(1), "sid").unwrap();
        remove_from_disk_in(root.path(), "sid").unwrap();
        remove_from_disk_in(root.path(), "sid").unwrap();
        assert!(from_disk_in::<Kata>(root.path(), "sid").is_err());
        assert!(!root.path().join("sid").exists());
    }

    #[test]
    fn missing_state_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert!(from_disk_in::<Kata>(root.path(), "sid").is_err());
    }
}
