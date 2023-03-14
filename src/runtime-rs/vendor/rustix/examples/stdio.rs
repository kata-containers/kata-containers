//! A command which prints out information about the standard input,
//! output, and error streams provided to it.

#![cfg_attr(io_lifetimes_use_std, feature(io_safety))]

#[cfg(not(windows))]
use rustix::fd::AsFd;
#[cfg(any(all(linux_raw, feature = "procfs"), all(not(windows), libc)))]
use rustix::io::ttyname;
#[cfg(not(windows))]
use rustix::io::{self, isatty, stderr, stdin, stdout};

#[cfg(not(windows))]
fn main() -> io::Result<()> {
    let (stdin, stdout, stderr) = unsafe { (stdin(), stdout(), stderr()) };

    println!("Stdin:");
    show(&stdin)?;

    println!("Stdout:");
    show(&stdout)?;

    println!("Stderr:");
    show(&stderr)?;

    Ok(())
}

#[cfg(not(windows))]
fn show<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    let fd = fd.as_fd();
    if isatty(fd) {
        #[cfg(any(all(linux_raw, feature = "procfs"), libc))]
        println!(" - ttyname: {}", ttyname(fd, Vec::new())?.to_string_lossy());
        println!(" - attrs: {:?}", rustix::io::ioctl_tcgets(fd)?);
        println!(" - winsize: {:?}", rustix::io::ioctl_tiocgwinsz(fd)?);
        println!(" - ready: {:?}", rustix::io::ioctl_fionread(fd)?);
    } else {
        println!("Stderr is not a tty");
    }
    Ok(())
}

#[cfg(windows)]
fn main() {
    unimplemented!()
}
