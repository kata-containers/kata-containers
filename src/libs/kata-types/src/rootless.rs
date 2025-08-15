// Copyright 2024 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{env, sync::Mutex};

use lazy_static::lazy_static;

lazy_static! {
    static ref ROOTLESS_STATE: Mutex<bool> = Mutex::new(false);
    static ref ROOTLESS_DIR: String = env::var("XDG_RUNTIME_DIR").unwrap_or_default();
}

/// Set the rootless state of vmm.
pub fn set_rootless(rootless: bool) {
    *ROOTLESS_STATE.lock().unwrap() = rootless;
}

/// Check whether the current vmm's state is rootless.
pub fn is_rootless() -> bool {
    *ROOTLESS_STATE.lock().unwrap()
}

/// Get the rootless directory's path of rootless vmm.
pub fn rootless_dir() -> String {
    ROOTLESS_DIR.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_rootless() {
        set_rootless(true);
        assert!(is_rootless());

        set_rootless(false);
        assert!(!is_rootless());
    }

    #[test]
    fn test_rootless_dir() {
        let temp_dir = env::temp_dir().to_str().unwrap().to_string();
        env::set_var("XDG_RUNTIME_DIR", temp_dir.as_str());
        assert_eq!(rootless_dir(), temp_dir);
    }
}
