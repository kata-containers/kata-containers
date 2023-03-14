use crate::{FileWrapper, ProcResult};

use std::{collections::HashMap, io::Read};

/// Represents the data from `/proc/cpuinfo`.
///
/// The `fields` field stores the fields that are common among all CPUs.  The `cpus` field stores
/// CPU-specific info.
///
/// For common fields, there are methods that will return the data, converted to a more appropriate
/// data type.  These methods will all return `None` if the field doesn't exist, or is in some
/// unexpected format (in that case, you'll have to access the string data directly).
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// This stores fields that are common among all CPUs
    pub fields: HashMap<String, String>,
    pub cpus: Vec<HashMap<String, String>>,
}

impl CpuInfo {
    /// Get CpuInfo from a custom Read instead of the default `/proc/cpuinfo`.
    pub fn from_reader<R: Read>(r: R) -> ProcResult<CpuInfo> {
        use std::io::{BufRead, BufReader};

        let reader = BufReader::new(r);

        let mut list = Vec::new();
        let mut map = Some(HashMap::new());

        // the first line of a cpu block must start with "processor"
        let mut found_first = false;

        for line in reader.lines().flatten() {
            if !line.is_empty() {
                let mut s = line.split(':');
                let key = expect!(s.next());
                if !found_first && key.trim() == "processor" {
                    found_first = true;
                }
                if !found_first {
                    continue;
                }
                if let Some(value) = s.next() {
                    let key = key.trim().to_owned();
                    let value = value.trim().to_owned();

                    map.get_or_insert(HashMap::new()).insert(key, value);
                }
            } else if let Some(map) = map.take() {
                list.push(map);
                found_first = false;
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
                if list.iter().all(|map| map.get(key).map_or(false, |v| v == val)) {
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
    pub fn new() -> ProcResult<CpuInfo> {
        let file = FileWrapper::open("/proc/cpuinfo")?;

        CpuInfo::from_reader(file)
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
        self.get_info(cpu_num).and_then(|mut m| m.remove("model name"))
    }
    pub fn vendor_id(&self, cpu_num: usize) -> Option<&str> {
        self.get_info(cpu_num).and_then(|mut m| m.remove("vendor_id"))
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

    #[test]
    fn test_cpuinfo_rpi() {
        // My rpi system includes some stuff at the end of /proc/cpuinfo that we shouldn't parse
        let data = r#"processor       : 0
model name      : ARMv7 Processor rev 4 (v7l)
BogoMIPS        : 38.40
Features        : half thumb fastmult vfp edsp neon vfpv3 tls vfpv4 idiva idivt vfpd32 lpae evtstrm crc32
CPU implementer : 0x41
CPU architecture: 7
CPU variant     : 0x0
CPU part        : 0xd03
CPU revision    : 4

processor       : 1
model name      : ARMv7 Processor rev 4 (v7l)
BogoMIPS        : 38.40
Features        : half thumb fastmult vfp edsp neon vfpv3 tls vfpv4 idiva idivt vfpd32 lpae evtstrm crc32
CPU implementer : 0x41
CPU architecture: 7
CPU variant     : 0x0
CPU part        : 0xd03
CPU revision    : 4

processor       : 2
model name      : ARMv7 Processor rev 4 (v7l)
BogoMIPS        : 38.40
Features        : half thumb fastmult vfp edsp neon vfpv3 tls vfpv4 idiva idivt vfpd32 lpae evtstrm crc32
CPU implementer : 0x41
CPU architecture: 7
CPU variant     : 0x0
CPU part        : 0xd03
CPU revision    : 4

processor       : 3
model name      : ARMv7 Processor rev 4 (v7l)
BogoMIPS        : 38.40
Features        : half thumb fastmult vfp edsp neon vfpv3 tls vfpv4 idiva idivt vfpd32 lpae evtstrm crc32
CPU implementer : 0x41
CPU architecture: 7
CPU variant     : 0x0
CPU part        : 0xd03
CPU revision    : 4

Hardware        : BCM2835
Revision        : a020d3
Serial          : 0000000012345678
Model           : Raspberry Pi 3 Model B Plus Rev 1.3
"#;

        let r = std::io::Cursor::new(data.as_bytes());

        let info = CpuInfo::from_reader(r).unwrap();
        assert_eq!(info.num_cores(), 4);
        let info = info.get_info(0).unwrap();
        assert!(info.get("model name").is_some());
        assert!(info.get("BogoMIPS").is_some());
        assert!(info.get("Features").is_some());
        assert!(info.get("CPU implementer").is_some());
        assert!(info.get("CPU architecture").is_some());
        assert!(info.get("CPU variant").is_some());
        assert!(info.get("CPU part").is_some());
        assert!(info.get("CPU revision").is_some());
    }
}
