//! A command which prints out information about the process it runs in.

#[cfg(not(windows))]
use rustix::io;
#[cfg(not(windows))]
use rustix::process::*;

#[cfg(not(windows))]
fn main() -> io::Result<()> {
    println!("Pid: {}", getpid().as_raw_nonzero());
    println!("Parent Pid: {}", Pid::as_raw(getppid()));
    println!("Uid: {}", getuid().as_raw());
    println!("Gid: {}", getgid().as_raw());
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        let (a, b) = linux_hwcap();
        println!("Linux hwcap: {:#x}, {:#x}", a, b);
    }
    println!("Page size: {}", page_size());
    println!("Clock ticks/sec: {}", clock_ticks_per_second());
    println!("Uname: {:?}", uname());
    println!("Process group priority: {}", getpriority_pgrp(None)?);
    println!("Process priority: {}", getpriority_process(None)?);
    println!("User priority: {}", getpriority_user(Uid::ROOT)?);
    println!(
        "Current working directory: {}",
        getcwd(Vec::new())?.to_string_lossy()
    );
    println!("Cpu Limit: {:?}", getrlimit(Resource::Cpu));
    println!("Fsize Limit: {:?}", getrlimit(Resource::Fsize));
    println!("Data Limit: {:?}", getrlimit(Resource::Data));
    println!("Stack Limit: {:?}", getrlimit(Resource::Stack));
    println!("Core Limit: {:?}", getrlimit(Resource::Core));
    println!("Rss Limit: {:?}", getrlimit(Resource::Rss));
    println!("Nproc Limit: {:?}", getrlimit(Resource::Nproc));
    println!("Nofile Limit: {:?}", getrlimit(Resource::Nofile));
    println!("Memlock Limit: {:?}", getrlimit(Resource::Memlock));
    #[cfg(not(target_os = "openbsd"))]
    println!("As Limit: {:?}", getrlimit(Resource::As));
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
    )))]
    println!("Locks Limit: {:?}", getrlimit(Resource::Locks));
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
    )))]
    println!("Sigpending Limit: {:?}", getrlimit(Resource::Sigpending));
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
    )))]
    println!("Msgqueue Limit: {:?}", getrlimit(Resource::Msgqueue));
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
    )))]
    println!("Nice Limit: {:?}", getrlimit(Resource::Nice));
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
    )))]
    println!("Rtprio Limit: {:?}", getrlimit(Resource::Rtprio));
    #[cfg(not(any(
        target_os = "emscripten",
        target_os = "freebsd",
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
    )))]
    println!("Rttime Limit: {:?}", getrlimit(Resource::Rttime));
    #[cfg(any(target_os = "android", target_os = "linux"))]
    println!("Execfn: {:?}", linux_execfn());
    Ok(())
}

#[cfg(windows)]
fn main() {
    unimplemented!()
}
