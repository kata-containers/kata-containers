// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod sandbox_persist;
use anyhow::{anyhow, Context, Ok, Result};
use kata_types::config::KATA_PATH;
use serde::de;
use std::{fs, fs::File, io::BufReader, io::ErrorKind, path::Path};

pub const PERSIST_FILE: &str = "state.json";
use kata_sys_util::validate::verify_id;
use safe_path::scoped_join;

/// Root recovery trusts this state, so the VMM user must not be able to replace it.
pub fn to_disk<T: serde::Serialize>(value: &T, sid: &str, jailer_path: &str) -> Result<()> {
    verify_id(sid).context("failed to verify sid")?;
    // FIXME: handle jailed case
    let mut path = match jailer_path {
        "" => scoped_join(KATA_PATH, sid)?,
        _ => scoped_join(jailer_path, "root")?,
    };
    //let mut path = scoped_join(KATA_PATH, sid)?;
    if path.exists() {
        path.push(PERSIST_FILE);
        let f = File::create(path)
            .context("failed to create the file")
            .context("failed to join the path")?;
        let j = serde_json::to_value(value).context("failed to convert to the json value")?;
        serde_json::to_writer_pretty(f, &j)?;
        return Ok(());
    }
    Err(anyhow!("invalid sid {}", sid))
}

pub fn from_disk<T>(sid: &str) -> Result<T>
where
    T: de::DeserializeOwned,
{
    verify_id(sid).context("failed to verify sid")?;
    let mut path = scoped_join(KATA_PATH, sid)?;
    if path.exists() {
        path.push(PERSIST_FILE);
        let file = File::open(path).context("failed to open the file")?;
        let reader = BufReader::new(file);
        return serde_json::from_reader(reader).map_err(|e| anyhow!(e.to_string()));
    }
    Err(anyhow!("invalid sid {}", sid))
}

/// A fresh Delete process needs this record to find the VMM user's runtime files.
pub const ROOTLESS_UID_FILE: &str = "rootless_uid";

pub fn record_rootless_uid(sid: &str, uid: u32) -> Result<()> {
    write_rootless_uid(Path::new(KATA_PATH), sid, uid)
}

pub fn recorded_rootless_uid(sid: &str) -> Result<Option<u32>> {
    read_rootless_uid(Path::new(KATA_PATH), sid)
}

pub fn has_cleanup_record(sid: &str) -> bool {
    cleanup_record_in(Path::new(KATA_PATH), sid)
}

fn cleanup_record_in(base: &Path, sid: &str) -> bool {
    if verify_id(sid).is_err() {
        return false;
    }
    match scoped_join(base, sid) {
        std::result::Result::Ok(dir) => {
            dir.join(PERSIST_FILE).exists() || dir.join(ROOTLESS_UID_FILE).exists()
        }
        Err(_) => false,
    }
}

fn write_rootless_uid(base: &Path, sid: &str, uid: u32) -> Result<()> {
    verify_id(sid).context("failed to verify sid")?;
    let dir = scoped_join(base, sid)?;
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(ROOTLESS_UID_FILE);
    // A partial UID record would make the sandbox undiscoverable during recovery.
    let tmp = dir.join(format!(".{ROOTLESS_UID_FILE}.tmp"));
    fs::write(&tmp, uid.to_string()).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("rename to {}", path.display()))
}

fn read_rootless_uid(base: &Path, sid: &str) -> Result<Option<u32>> {
    verify_id(sid).context("failed to verify sid")?;
    let path = scoped_join(base, sid)?.join(ROOTLESS_UID_FILE);
    match fs::read_to_string(&path) {
        std::result::Result::Ok(uid) => uid
            .trim()
            .parse()
            .map(Some)
            .with_context(|| format!("parse {}", path.display())),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        cleanup_record_in, from_disk, read_rootless_uid, to_disk, write_rootless_uid, KATA_PATH,
        PERSIST_FILE,
    };
    use serde::{Deserialize, Serialize};
    use std::fs::DirBuilder;
    use std::{fs, result::Result::Ok};

    #[test]
    fn test_rootless_uid_record() {
        let base = tempfile::tempdir().unwrap();
        let sid = "rootless-sandbox";
        assert_eq!(read_rootless_uid(base.path(), sid).unwrap(), None);

        write_rootless_uid(base.path(), sid, 1001).unwrap();
        assert_eq!(read_rootless_uid(base.path(), sid).unwrap(), Some(1001));

        fs::write(
            base.path().join(sid).join(super::ROOTLESS_UID_FILE),
            "../1001",
        )
        .unwrap();
        assert!(read_rootless_uid(base.path(), sid).is_err());
        assert!(write_rootless_uid(base.path(), "../escape", 1001).is_err());

        write_rootless_uid(base.path(), sid, 1002).unwrap();
        assert_eq!(read_rootless_uid(base.path(), sid).unwrap(), Some(1002));
        let entries: Vec<_> = fs::read_dir(base.path().join(sid))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries, [super::ROOTLESS_UID_FILE]);
    }

    #[test]
    fn test_cleanup_record() {
        let base = tempfile::tempdir().unwrap();
        let sid = "sandbox";
        assert!(!cleanup_record_in(base.path(), sid));
        fs::create_dir(base.path().join(sid)).unwrap();
        assert!(!cleanup_record_in(base.path(), sid));

        // Preserve an invalid record so it can be repaired before retrying recovery.
        fs::write(base.path().join(sid).join(super::ROOTLESS_UID_FILE), "x").unwrap();
        assert!(cleanup_record_in(base.path(), sid));
        fs::remove_file(base.path().join(sid).join(super::ROOTLESS_UID_FILE)).unwrap();

        fs::write(base.path().join(sid).join(PERSIST_FILE), "{}").unwrap();
        assert!(cleanup_record_in(base.path(), sid));
        assert!(!cleanup_record_in(base.path(), "../sandbox"));
    }

    #[test]
    fn test_to_from_disk() {
        #[derive(Serialize, Deserialize, Debug)]
        struct Kata {
            name: String,
            key: u8,
        }
        let data = Kata {
            name: "kata".to_string(),
            key: 1,
        };
        // invalid sid
        assert!(to_disk(&data, "..3", "").is_err());
        assert!(to_disk(&data, "../../../3", "").is_err());
        assert!(to_disk(&data, "a/b/c", "").is_err());
        assert!(to_disk(&data, ".#cdscd.", "").is_err());

        let sid = "aadede";
        let sandbox_dir = [KATA_PATH, sid].join("/");
        if DirBuilder::new()
            .recursive(true)
            .create(&sandbox_dir)
            .is_ok()
        {
            assert!(to_disk(&data, sid, "").is_ok());
            if let Ok(result) = from_disk::<Kata>(sid) {
                assert_eq!(result.name, data.name);
                assert_eq!(result.key, data.key);
            }
            assert!(fs::remove_dir_all(&sandbox_dir).is_ok());
        }
    }
}
