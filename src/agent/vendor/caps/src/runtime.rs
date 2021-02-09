//! Detect kernel features at runtime.
//!
//! This module exposes methods to perform detection of kernel
//! features at runtime. This allows applications to auto-detect
//! whether recent options are implemented by the currently
//! running kernel.

use super::{ambient, CapSet, Capability, CapsHashSet};
use errors::*;

/// Check whether the running kernel supports the ambient set.
///
/// Ambient set was introduced in Linux kernel 4.3. On recent kernels
/// where the ambient set is supported, this will return `Ok`.
/// On a legacy kernel, an `Err` is returned instead.
pub fn ambient_set_supported() -> Result<()> {
    ambient::has_cap(Capability::CAP_CHOWN)?;
    Ok(())
}

/// Return an `HashSet` with all capabilities supported by the running kernel.
pub fn all_supported() -> CapsHashSet {
    let mut supported = super::all();
    for c in super::all() {
        if super::has_cap(None, CapSet::Bounding, c).is_err() {
            supported.remove(&c);
        }
    }
    supported
}
