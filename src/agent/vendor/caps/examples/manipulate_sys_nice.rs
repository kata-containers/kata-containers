//! A simple example showing how to manipulate capabilities.
//!
//! It drops and raises `CAP_SYS_NICE` to show its interaction
//! with `getpriority(2)`.
//!
//! This is a caps example ONLY: do NOT panic/unwrap/assert
//! in production code!

extern crate caps;
extern crate libc;

use caps::{CapSet, Capability};

fn main() {
    // Any process can lower its own priority.
    println!("-> Current process priority is {}.", proc_nice());
    let r = renice(19);
    assert_eq!(r, 0);
    println!("Lowered priority to +19.");
    println!("-> Current process priority is {}.", proc_nice());

    // Without `CAP_SYS_NICE` increasing priority is not possible.
    let r = caps::drop(None, CapSet::Effective, Capability::CAP_SYS_NICE);
    assert!(r.is_ok());
    println!("Dropped CAP_SYS_NICE.");
    let has_sys_nice = caps::has_cap(None, CapSet::Effective, Capability::CAP_SYS_NICE);
    assert!(has_sys_nice.is_ok());
    assert_eq!(has_sys_nice.unwrap(), false);
    let r = renice(-20);
    assert_eq!(r, -1);
    println!("Unprivileged, unable to raise priority to -20.");

    // If `CAP_SYS_NICE` is still in permitted set, it can be raised again.
    let perm_sys_nice = caps::has_cap(None, CapSet::Permitted, Capability::CAP_SYS_NICE);
    assert!(perm_sys_nice.is_ok());
    if !perm_sys_nice.unwrap() {
        println!("Try running this again as root/sudo or with CAP_SYS_NICE file capability!");
        return;
    }
    let r = caps::raise(None, CapSet::Effective, Capability::CAP_SYS_NICE);
    assert!(r.is_ok());
    println!("Raised CAP_SYS_NICE.");

    // With CAP_SYS_NICE, priority can be raised further.
    let r = renice(-20);
    assert_eq!(r, 0);
    println!("Privileged, raised priority to -20.");
    println!("-> Current process priority is {}.", proc_nice());
}

#[cfg(target_env = "musl")]
const PRIO_PROCESS: i32 = libc::PRIO_PROCESS;
#[cfg(not(target_env = "musl"))]
const PRIO_PROCESS: u32 = libc::PRIO_PROCESS as u32;

fn renice(prio: libc::c_int) -> libc::c_int {
    // This is not proper logic, as it does not drain errno.
    unsafe { libc::setpriority(PRIO_PROCESS, 0, prio) }
}

fn proc_nice() -> libc::c_int {
    // This is not proper logic, as it does not special-case -1 nor drain errno.
    let r = unsafe { libc::getpriority(PRIO_PROCESS as u32, 0) };
    if r == -1 {
        panic!("getpriority failed.");
    }
    r
}
