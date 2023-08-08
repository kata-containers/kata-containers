// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// Copyright © 2020 Intel Corporation
//
// Copyright © 2021 Ant Group Corporation

// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

use log::error;

pub struct EpollHelper {
    epoll_file: File,
}

#[derive(Debug)]
pub enum EpollHelperError {
    CreateFd(std::io::Error),
    Ctl(std::io::Error),
    IoError(std::io::Error),
    Wait(std::io::Error),
}

pub trait EpollHelperHandler {
    // Return true if execution of the loop should be stopped
    fn handle_event(&mut self, helper: &mut EpollHelper, event: &epoll::Event) -> bool;
}

impl EpollHelper {
    pub fn new() -> std::result::Result<Self, EpollHelperError> {
        // Create the epoll file descriptor
        let epoll_fd = epoll::create(true).map_err(EpollHelperError::CreateFd)?;
        // Use 'File' to enforce closing on 'epoll_fd'
        let epoll_file = unsafe { File::from_raw_fd(epoll_fd) };

        Ok(Self { epoll_file })
    }

    pub fn add_event(&mut self, fd: RawFd, id: u32) -> std::result::Result<(), EpollHelperError> {
        self.add_event_custom(fd, id, epoll::Events::EPOLLIN)
    }

    pub fn add_event_custom(
        &mut self,
        fd: RawFd,
        id: u32,
        evts: epoll::Events,
    ) -> std::result::Result<(), EpollHelperError> {
        epoll::ctl(
            self.epoll_file.as_raw_fd(),
            epoll::ControlOptions::EPOLL_CTL_ADD,
            fd,
            epoll::Event::new(evts, id.into()),
        )
        .map_err(EpollHelperError::Ctl)
    }

    pub fn del_event_custom(
        &mut self,
        fd: RawFd,
        id: u32,
        evts: epoll::Events,
    ) -> std::result::Result<(), EpollHelperError> {
        epoll::ctl(
            self.epoll_file.as_raw_fd(),
            epoll::ControlOptions::EPOLL_CTL_DEL,
            fd,
            epoll::Event::new(evts, id.into()),
        )
        .map_err(EpollHelperError::Ctl)
    }

    pub fn run(
        &mut self,
        handler: &mut dyn EpollHelperHandler,
    ) -> std::result::Result<(), EpollHelperError> {
        const EPOLL_EVENTS_LEN: usize = 100;
        let mut events = vec![epoll::Event::new(epoll::Events::empty(), 0); EPOLL_EVENTS_LEN];

        loop {
            let num_events = match epoll::wait(self.epoll_file.as_raw_fd(), -1, &mut events[..]) {
                Ok(res) => res,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        // It's well defined from the epoll_wait() syscall
                        // documentation that the epoll loop can be interrupted
                        // before any of the requested events occurred or the
                        // timeout expired. In both those cases, epoll_wait()
                        // returns an error of type EINTR, but this should not
                        // be considered as a regular error. Instead it is more
                        // appropriate to retry, by calling into epoll_wait().
                        continue;
                    }
                    error!("io thread epoll wait failed: {:?}", e);
                    return Err(EpollHelperError::Wait(e));
                }
            };

            for event in events.iter().take(num_events) {
                if handler.handle_event(self, event) {
                    return Ok(());
                }
            }
        }
    }
}

impl AsRawFd for EpollHelper {
    fn as_raw_fd(&self) -> RawFd {
        self.epoll_file.as_raw_fd()
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::io::AsRawFd;
    use vmm_sys_util::eventfd::EventFd;

    use super::EpollHelper;

    #[test]
    fn test_new_epoller() {
        let helper = EpollHelper::new();
        assert!(helper.is_ok());
    }

    #[test]
    fn test_add_event() {
        let helper = EpollHelper::new();
        assert!(helper.is_ok());

        let eventfd = EventFd::new(0).unwrap();

        let res = helper.unwrap().add_event(eventfd.as_raw_fd(), 0);
        assert!(res.is_ok())
    }

    #[test]
    fn test_delete_event() {
        let helper = EpollHelper::new();
        assert!(helper.is_ok());

        let eventfd = EventFd::new(0).unwrap();
        let mut helper = helper.unwrap();
        let res = helper.add_event(eventfd.as_raw_fd(), 0);
        assert!(res.is_ok());

        let res = helper.del_event_custom(eventfd.as_raw_fd(), 0, epoll::Events::EPOLLIN);
        assert!(res.is_ok());
    }
}
