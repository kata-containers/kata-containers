use super::{MemoryMap, MemoryMapData};
use crate::{ProcError, ProcResult};
use std::io::{BufRead, BufReader, Read};

#[derive(Debug)]
pub struct SmapsRollup {
    pub memory_map: MemoryMap,
    pub memory_map_data: MemoryMapData,
}

impl SmapsRollup {
    // this implemenation is similar but not identical to Process::smaps()
    pub fn from_reader<R: Read>(r: R) -> ProcResult<SmapsRollup> {
        let reader = BufReader::new(r);

        let mut memory_map = MemoryMap::new();
        let mut memory_map_data: MemoryMapData = Default::default();
        let mut first = true;
        for line in reader.lines() {
            let line = line.map_err(|_| ProcError::Incomplete(None))?;

            if first {
                memory_map = MemoryMap::from_line(&line)?;
                first = false;
                continue;
            }

            let mut parts = line.split_ascii_whitespace();

            let key = parts.next();
            let value = parts.next();

            if let (Some(k), Some(v)) = (key, value) {
                // While most entries do have one, not all of them do.
                let size_suffix = parts.next();

                // Limited poking at /proc/<pid>/smaps and then checking if "MB", "GB", and "TB" appear in the C file that is
                // supposedly responsible for creating smaps, has lead me to believe that the only size suffixes we'll ever encounter
                // "kB", which is most likely kibibytes. Actually checking if the size suffix is any of the above is a way to
                // future-proof the code, but I am not sure it is worth doing so.
                let size_multiplier = if size_suffix.is_some() { 1024 } else { 1 };

                let v = v
                    .parse::<u64>()
                    .map_err(|_| ProcError::Other("Value in `Key: Value` pair was not actually a number".into()))?;

                // This ignores the case when our Key: Value pairs are really Key Value pairs. Is this a good idea?
                let k = k.trim_end_matches(':');

                memory_map_data.map.insert(k.into(), v * size_multiplier);
            }
        }
        Ok(SmapsRollup {
            memory_map,
            memory_map_data,
        })
    }
}
