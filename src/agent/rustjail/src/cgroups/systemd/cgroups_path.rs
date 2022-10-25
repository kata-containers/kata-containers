// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};

use super::common::{DEFAULT_SLICE, SCOPE_SUFFIX, SLICE_SUFFIX};
use std::string::String;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CgroupsPath {
    pub slice: String,
    pub prefix: String,
    pub name: String,
}

impl CgroupsPath {
    pub fn new(cgroups_path_str: &str) -> Result<Self> {
        let path_vec: Vec<&str> = cgroups_path_str.split(':').collect();
        if path_vec.len() != 3 {
            return Err(anyhow!("invalid cpath: {:?}", cgroups_path_str));
        }

        Ok(CgroupsPath {
            slice: if path_vec[0].is_empty() {
                DEFAULT_SLICE.to_string()
            } else {
                path_vec[0].to_owned()
            },
            prefix: path_vec[1].to_owned(),
            name: path_vec[2].to_owned(),
        })
    }

    // ref: https://github.com/opencontainers/runc/blob/main/docs/systemd.md
    // return: (parent_slice, unit_name)
    pub fn parse(&self) -> Result<(String, String)> {
        Ok((
            parse_parent(self.slice.to_owned())?,
            get_unit_name(self.prefix.to_owned(), self.name.to_owned()),
        ))
    }
}

fn parse_parent(slice: String) -> Result<String> {
    if !slice.ends_with(SLICE_SUFFIX) || slice.contains('/') {
        return Err(anyhow!("invalid slice name: {}", slice));
    } else if slice == "-.slice" {
        return Ok(String::new());
    }

    let mut slice_path = String::new();
    let mut prefix = String::new();
    for subslice in slice.trim_end_matches(SLICE_SUFFIX).split('-') {
        if subslice.is_empty() {
            return Err(anyhow!("invalid slice name: {}", slice));
        }
        slice_path = format!("{}/{}{}{}", slice_path, prefix, subslice, SLICE_SUFFIX);
        prefix = format!("{}{}-", prefix, subslice);
    }
    slice_path.remove(0);
    Ok(slice_path)
}

fn get_unit_name(prefix: String, name: String) -> String {
    if name.ends_with(SLICE_SUFFIX) {
        name
    } else if prefix.is_empty() {
        format!("{}{}", name, SCOPE_SUFFIX)
    } else {
        format!("{}-{}{}", prefix, name, SCOPE_SUFFIX)
    }
}

#[cfg(test)]
mod tests {
    use super::CgroupsPath;

    #[test]
    fn test_cgroup_path_parse() {
        let slice = "system.slice";
        let prefix = "kata_agent";
        let name = "123";
        let cgroups_path =
            CgroupsPath::new(format!("{}:{}:{}", slice, prefix, name).as_str()).unwrap();
        assert_eq!(slice, cgroups_path.slice.as_str());
        assert_eq!(prefix, cgroups_path.prefix.as_str());
        assert_eq!(name, cgroups_path.name.as_str());

        let (parent_slice, unit_name) = cgroups_path.parse().unwrap();
        assert_eq!(format!("{}", slice), parent_slice);
        assert_eq!(format!("{}-{}.scope", prefix, name), unit_name);
    }
}
