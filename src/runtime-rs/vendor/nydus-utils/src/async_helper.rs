// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
use tokio::runtime::{Builder, Runtime};

std::thread_local! {
    static CURRENT_THREAD_RT: Runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("utils: failed to create tokio runtime for current thread");
}

/// Run the callback with a tokio current thread runtime instance.
pub fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce(&Runtime) -> R,
{
    CURRENT_THREAD_RT.with(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_runtime() {
        let res = with_runtime(|rt| rt.block_on(async { 1 }));
        assert_eq!(res, 1);

        let res = with_runtime(|rt| rt.block_on(async { 3 }));
        assert_eq!(res, 3);
    }
}
