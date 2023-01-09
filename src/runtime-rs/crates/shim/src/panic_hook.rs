// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{boxed::Box, fs::OpenOptions, io::Write, ops::Deref};

use backtrace::Backtrace;

const KMESG_DEVICE: &str = "/dev/kmsg";

// TODO: the Kata 1.x runtime had a SIGUSR1 handler that would log a formatted backtrace on
// receiving that signal. It could be useful to re-add that feature.
pub(crate) fn set_panic_hook() {
    std::panic::set_hook(Box::new(move |panic_info| {
        let (filename, line) = panic_info
            .location()
            .map(|loc| (loc.file(), loc.line()))
            .unwrap_or(("<unknown>", 0));

        let cause = panic_info
            .payload()
            .downcast_ref::<String>()
            .map(std::string::String::deref);

        let cause = cause.unwrap_or_else(|| {
            panic_info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .unwrap_or("<cause unknown>")
        });
        let bt = Backtrace::new();
        let bt_data = format!("{:?}", bt);
        error!(
            sl!(),
            "A panic occurred at {}:{}: {}\r\n{:?}", filename, line, cause, bt_data
        );

        // print panic log to dmesg
        // The panic log size is too large to /dev/kmsg, so write by line.
        if let Ok(mut file) = OpenOptions::new().write(true).open(KMESG_DEVICE) {
            file.write_all(
                format!("A panic occurred at {}:{}: {}", filename, line, cause).as_bytes(),
            )
            .ok();
            let lines: Vec<&str> = bt_data.split('\n').collect();
            for line in lines {
                file.write_all(line.as_bytes()).ok();
            }

            file.flush().ok();
        }
        std::process::abort();
    }));
}
