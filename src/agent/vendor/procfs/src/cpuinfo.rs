use crate::ProcResult;

use std::collections::HashMap;

/// Represents the data from `/proc/cpuinfo`.
///
/// The `fields` field stores the fields that are common among all CPUs.  The `cpus` field stores
/// CPU-specific info.
///
/// For common fields, there are methods that will return the data, converted to a more appropriate
/// data type.  These methods will all return `None` if the field doesn't exist, or is in some
/// unexpected format (in that case, you'll have to access the string data directly).
#[derive(Debug)]
pub struct CpuInfo {
    /// This stores fields that are common among all CPUs
    pub fields: HashMap<String, String>,
    pub cpus: Vec<HashMap<String, String>>,
}

impl CpuInfo {
    pub fn new() -> ProcResult<CpuInfo> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let file = File::open("/proc/cpuinfo")?;
        let reader = BufReader::new(file);

        let mut list = Vec::new();
        let mut map = Some(HashMap::new());

        for line in reader.lines() {
            if let Ok(line) = line {
                if !line.is_empty() {
                    let mut s = line.split(':');
                    let key = expect!(s.next());
                    if let Some(value) = s.next() {
                        let key = key.trim().to_owned();
                        let value = value.trim().to_owned();

                        map.get_or_insert(HashMap::new()).insert(key, value);
                    }
                } else if let Some(map) = map.take() {
                    list.push(map);
                }
            }
        }
        if let Some(map) = map.take() {
            list.push(map);
        }

        // find properties that are the same for all cpus
        assert!(!list.is_empty());

        let common_fields: Vec<String> = list[0]
            .iter()
            .filter_map(|(key, val)| {
                if list
                    .iter()
                    .all(|map| map.get(key).map_or(false, |v| v == val))
                {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        let mut common_map = HashMap::new();
        for (k, v) in &list[0] {
            if common_fields.contains(k) {
                common_map.insert(k.clone(), v.clone());
            }
        }

        for map in &mut list {
            map.retain(|k, _| !common_fields.contains(k));
        }

        Ok(CpuInfo {
            fields: common_map,
            cpus: list,
        })
    }

    /// Get the total number of cpu cores.
    ///
    /// This is the number of entries in the `/proc/cpuinfo` file.
    pub fn num_cores(&self) -> usize {
        self.cpus.len()
    }

    /// Get info for a specific cpu.
    ///
    /// This will merge the common fields with the cpu-specific fields.
    ///
    /// Returns None if the requested cpu index is not found.
    pub fn get_info(&self, cpu_num: usize) -> Option<HashMap<&str, &str>> {
        if let Some(info) = self.cpus.get(cpu_num) {
            let mut map = HashMap::new();

            for (k, v) in &self.fields {
                map.insert(k.as_ref(), v.as_ref());
            }

            for (k, v) in info.iter() {
                map.insert(k.as_ref(), v.as_ref());
            }

            Some(map)
        } else {
            None
        }
    }

    pub fn model_name(&self, cpu_num: usize) -> Option<&str> {
        self.get_info(cpu_num)
            .and_then(|mut m| m.remove("model name"))
    }
    pub fn vendor_id(&self, cpu_num: usize) -> Option<&str> {
        self.get_info(cpu_num)
            .and_then(|mut m| m.remove("vendor_id"))
    }
    /// May not be available on some older 2.6 kernels
    pub fn physical_id(&self, cpu_num: usize) -> Option<u32> {
        self.get_info(cpu_num)
            .and_then(|mut m| m.remove("physical id"))
            .and_then(|s| u32::from_str_radix(s, 10).ok())
    }
    pub fn flags(&self, cpu_num: usize) -> Option<Vec<&str>> {
        self.get_info(cpu_num)
            .and_then(|mut m| m.remove("flags"))
            .map(|flags: &str| flags.split_whitespace().collect())
    }
}

/// Get CPU info, from /proc/cpuinfo
#[deprecated(note = "Please use the CpuInfo::new() method instead")]
pub fn cpuinfo() -> ProcResult<CpuInfo> {
    CpuInfo::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpuinfo() {
        let info = CpuInfo::new().unwrap();
        println!("{:#?}", info.flags(0));
        for num in 0..info.num_cores() {
            info.model_name(num).unwrap();
            info.vendor_id(num).unwrap();
            // May not be available on some old kernels:
            info.physical_id(num);
        }

        //assert_eq!(info.num_cores(), 8);
    }
}
