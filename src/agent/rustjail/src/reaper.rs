// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use nix::fcntl::OFlag;
use slog::Logger;

use nix::unistd;
use std::os::unix::io::RawFd;

use anyhow::Result;

const MAX_EVENTS: usize = 2;

#[derive(Debug, Clone)]
pub struct Epoller {
    logger: Logger,
    epoll_fd: RawFd,
    // rfd and wfd are a pipe's files two ends, this pipe is
    // used to sync between the readStdio and the process exits.
    // once the process exits, it will close one end to notify
    // the readStdio that the process has exited and it should not
    // wait on the process's terminal which has been inherited
    // by it's children and hasn't exited.
    rfd: RawFd,
    wfd: RawFd,
}

impl Epoller {
    pub fn new(logger: &Logger, fd: RawFd) -> Result<Epoller> {
        let epoll_fd = epoll::create(true)?;
        let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;

        let mut epoller = Self {
            logger: logger.clone(),
            epoll_fd,
            rfd,
            wfd,
        };

        epoller.add(rfd)?;
        epoller.add(fd)?;

        Ok(epoller)
    }

    pub fn close_wfd(&self) {
        let _ = unistd::close(self.wfd);
    }

    pub fn close(&self) {
        let _ = unistd::close(self.rfd);
        let _ = unistd::close(self.wfd);
        let _ = unistd::close(self.epoll_fd);
    }

    fn add(&mut self, fd: RawFd) -> Result<()> {
        info!(self.logger, "Epoller add fd {}", fd);
        // add creates an epoll which is used to monitor the process's pty's master and
        // one end of its exit notify pipe. Those files will be registered with level-triggered
        // notification.
        epoll::ctl(
            self.epoll_fd,
            epoll::ControlOptions::EPOLL_CTL_ADD,
            fd,
            epoll::Event::new(
                epoll::Events::EPOLLHUP
                    | epoll::Events::EPOLLIN
                    | epoll::Events::EPOLLERR
                    | epoll::Events::EPOLLRDHUP,
                fd as u64,
            ),
        )?;

        Ok(())
    }

    // There will be three cases on the epoller once it poll:
    // a: only pty's master get an event(other than self.rfd);
    // b: only the pipe get an event(self.rfd);
    // c: both of pty and pipe have event occur;
    // for case a, it means there is output in process's terminal and what needed to do is
    // just read the terminal and send them out; for case b, it means the process has exited
    // and there is no data in the terminal, thus just return the "EOF" to end the io;
    // for case c, it means the process has exited but there is some data in the terminal which
    // hasn't been send out, thus it should send those data out first and then send "EOF" last to
    // end the io.
    pub fn poll(&self) -> Result<RawFd> {
        let mut rfd = self.rfd;
        let mut epoll_events = vec![epoll::Event::new(epoll::Events::empty(), 0); MAX_EVENTS];

        loop {
            let event_count = match epoll::wait(self.epoll_fd, -1, epoll_events.as_mut_slice()) {
                Ok(ec) => ec,
                Err(e) => {
                    info!(self.logger, "loop wait err {:?}", e);
                    // EINTR: The call was interrupted by a signal handler before either
                    // any of the requested events occurred or the timeout expired
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(e.into());
                }
            };

            for event in epoll_events.iter().take(event_count) {
                let fd = event.data as i32;
                // fd has been assigned with one end of process's exited pipe by default, and
                // here to check is there any event occur on process's terminal, if "yes", it
                // should be dealt first, otherwise, it means the process has exited and there
                // is nothing left in the process's terminal needed to be read.
                if fd != rfd {
                    rfd = fd;
                    break;
                }
            }
            break;
        }

        Ok(rfd)
    }
}

#[cfg(test)]
mod tests {
    use super::Epoller;
    use nix::fcntl::OFlag;
    use nix::unistd;
    use std::thread;

    #[test]
    fn test_epoller_poll() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC).unwrap();
        let epoller = Epoller::new(&logger, rfd).unwrap();

        let child = thread::spawn(move || {
            let _ = unistd::write(wfd, "temporary file's content".as_bytes());
        });

        // wait write to finish
        let _ = child.join();

        let fd = epoller.poll().unwrap();
        assert_eq!(fd, rfd, "Should get rfd");

        epoller.close();
    }
}
