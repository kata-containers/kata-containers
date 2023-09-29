use std::path::Path;
use std::fs::File;


use std::boxed::Box;

use anyhow::Result;
use nix::fcntl::{OFlag, open};
use nix::pty::{grantpt, posix_openpt, ptsname, unlockpt};
use nix::sys::stat::Mode;
use termios::Termios;

fn new_pty() -> Result<(Termios, String)> {
    // Open a new PTY master
    let master_fd = posix_openpt(OFlag::O_RDWR)?;

    // Allow a slave to be generated for it
    grantpt(&master_fd)?;
    unlockpt(&master_fd)?;

    // Get the name of the slave
    let slave_name = unsafe { ptsname(&master_fd) }?;

    // Get the termios as the console to be used by cloud hypervisor
    let termios = Termios::from_fd(fd)?;

    // The termios will be used by ch, and the slave is the source of log
    Ok((termios, slave_name))
}

fn get_vm_console() -> Result<String> {
    // Log this behavior
    // TODO

    let (master, slave) = new_pty()?;

    // Set the ch's console to master
    // TODO

    Ok(slave)
}

pub(crate) struct ConsoleWatcher {
    console_url: String,
    pty_console: Option<File>,
}

impl ConsoleWatcher {
    fn new_console_watcher() -> Result<ConsoleWatcher> {
        let console_url = get_vm_console()?;
        Ok(ConsoleWatcher {
            console_url,
            pty_console: None,
        })
    }

    fn console_watched(&self) -> bool {
        self.pty_console.is_some()
    }

    fn start(&mut self) -> Result<()> {
        // Log this behavior
        // TODO
        if console_watched() {
            // Return an error
        }

        let f = File::open(self.console_url)?;
        self.pty_console = Some(f);

        // Read the content from pty_console
        // TODO

        Ok(())
    }

    fn stop(&mut self) {
        if self.pty_console.is_some() {
            self.pty_console.close();
            self.pty_console = None;
        }
    }
}