/*!
Detect kernel features at runtime.

This module exposes methods to perform detection of kernel
features at runtime. This allows applications to auto-detect
whether recent options are implemented by the currently
running kernel.

## Example

```rust
let ambient = caps::runtime::ambient_set_supported().is_ok();
println!("Supported ambient set: {}", ambient);

let all = caps::runtime::procfs_all_supported(None)
    .unwrap_or_else(|_| caps::runtime::thread_all_supported());
println!("Supported capabilities: {}", all.len());
```
!*/

use super::{ambient, CapSet, Capability, CapsHashSet};
use crate::errors::CapsError;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Check whether the running kernel supports the ambient set.
///
/// Ambient set was introduced in Linux kernel 4.3. On recent kernels
/// where the ambient set is supported, this will return `Ok`.
/// On a legacy kernel, an `Err` is returned instead.
pub fn ambient_set_supported() -> Result<(), CapsError> {
    ambient::has_cap(Capability::CAP_CHOWN)?;
    Ok(())
}

/// Return the set of all capabilities supported by the running kernel.
///
/// This requires a mounted `procfs` and a kernel version >= 3.2. By default,
/// it uses `/proc/` as the procfs mountpoint.
pub fn procfs_all_supported(proc_mountpoint: Option<PathBuf>) -> Result<CapsHashSet, CapsError> {
    /// See `man 2 capabilities`.
    const LAST_CAP_FILEPATH: &str = "./sys/kernel/cap_last_cap";
    let last_cap_path = proc_mountpoint
        .unwrap_or_else(|| PathBuf::from("/proc/"))
        .join(Path::new(LAST_CAP_FILEPATH));

    let max_cap: u8 = {
        let mut buf = String::with_capacity(4);
        std::fs::File::open(last_cap_path.clone())
            .and_then(|mut file| file.read_to_string(&mut buf))
            .map_err(|e| format!("failed to read '{}': {}", last_cap_path.display(), e))?;
        buf.trim_end()
            .parse()
            .map_err(|e| format!("failed to parse '{}': {}", last_cap_path.display(), e))?
    };

    let mut supported = super::all();
    for c in super::all() {
        if c.index() > max_cap {
            supported.remove(&c);
        }
    }
    Ok(supported)
}

/// Return the set of all capabilities supported on the current thread.
///
/// This does not require a mounted `procfs`, and it works with any
/// kernel version >= 2.6.25.
/// It internally uses `prctl(2)` and `PR_CAPBSET_READ`; if those are
/// unavailable, this will result in an empty set.
pub fn thread_all_supported() -> CapsHashSet {
    let mut supported = super::all();
    for c in super::all() {
        if super::has_cap(None, CapSet::Bounding, c).is_err() {
            supported.remove(&c);
        }
    }
    supported
}
