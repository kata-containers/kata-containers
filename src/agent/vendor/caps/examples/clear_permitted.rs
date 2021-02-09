//! A simple example showing how to manipulate capabilities.
//!
//! It clears Permitted set to show its interaction
//! with Effective set.
//!
//! This is an example ONLY: do NOT panic/unwrap/assert
//! in production code!

type ExResult<T> = Result<T, Box<dyn std::error::Error + 'static>>;

fn main() -> ExResult<()> {
    use caps::{CapSet, Capability};

    // Check if `CAP_CHOWN` was originally available.
    let cur = caps::read(None, CapSet::Permitted)?;
    println!("-> Current permitted caps: {:?}.", cur);
    let cur = caps::read(None, CapSet::Effective)?;
    println!("-> Current effective caps: {:?}.", cur);
    let perm_chown = caps::has_cap(None, CapSet::Permitted, Capability::CAP_CHOWN);
    assert!(perm_chown.is_ok());
    if !perm_chown? {
        return Err(
            "Try running this again as root/sudo or with CAP_CHOWN file capability!".into(),
        );
    }

    // Clear all effective caps.
    let r = caps::clear(None, CapSet::Effective);
    assert!(r.is_ok());
    println!("Cleared effective caps.");
    let cur = caps::read(None, CapSet::Effective)?;
    println!("-> Current effective caps: {:?}.", cur);

    // Since `CAP_CHOWN` is still in permitted, it can be raised again.
    let r = caps::raise(None, CapSet::Effective, Capability::CAP_CHOWN);
    assert!(r.is_ok());
    println!("Raised CAP_CHOWN in effective set.");
    let cur = caps::read(None, CapSet::Effective)?;
    println!("-> Current effective caps: {:?}.", cur);

    // Clearing Permitted also impacts effective.
    let r = caps::clear(None, CapSet::Permitted);
    assert!(r.is_ok());
    println!("Cleared permitted caps.");
    let cur = caps::read(None, CapSet::Permitted)?;
    println!("-> Current permitted caps: {:?}.", cur);
    let cur = caps::read(None, CapSet::Effective)?;
    println!("-> Current effective caps: {:?}.", cur);

    // Trying to raise `CAP_CHOWN` now fails.
    let r = caps::raise(None, CapSet::Effective, Capability::CAP_CHOWN);
    assert!(r.is_err());
    println!("Tried to raise CAP_CHOWN but failed.");
    let cur = caps::read(None, CapSet::Effective)?;
    println!("-> Current effective caps: {:?}.", cur);

    Ok(())
}
