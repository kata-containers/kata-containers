// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod sandbox_persist;
use anyhow::{anyhow, Context, Ok, Result};
use kata_types::config::KATA_PATH;
use serde::de;
use std::{fs::File, io::BufReader};

pub const PERSIST_FILE: &str = "state.json";
use kata_sys_util::validate::verify_id;
use safe_path::scoped_join;

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

#[cfg(test)]
mod tests {
    use crate::{from_disk, to_disk, KATA_PATH};
    use serde::{Deserialize, Serialize};
    use std::fs::DirBuilder;
    use std::{fs, result::Result::Ok};
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
