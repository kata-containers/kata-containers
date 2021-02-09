use std::fs;
#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
#[cfg(unix)]
use libc;
use finder::Checker;

pub struct ExecutableChecker;

impl ExecutableChecker {
    pub fn new() -> ExecutableChecker {
        ExecutableChecker
    }
}

impl Checker for ExecutableChecker {
    #[cfg(unix)]
    fn is_valid(&self, path: &Path) -> bool {
        CString::new(path.as_os_str().as_bytes())
            .and_then(|c| Ok(unsafe { libc::access(c.as_ptr(), libc::X_OK) == 0 }))
            .unwrap_or(false)
    }

    #[cfg(windows)]
    fn is_valid(&self, _path: &Path) -> bool {
        true
    }
}

pub struct ExistedChecker;

impl ExistedChecker {
    pub fn new() -> ExistedChecker {
        ExistedChecker
    }
}

impl Checker for ExistedChecker {
    fn is_valid(&self, path: &Path) -> bool {
        fs::metadata(path)
            .map(|metadata| metadata.is_file())
            .unwrap_or(false)
    }
}

pub struct CompositeChecker {
    checkers: Vec<Box<Checker>>,
}

impl CompositeChecker {
    pub fn new() -> CompositeChecker {
        CompositeChecker {
            checkers: Vec::new(),
        }
    }

    pub fn add_checker(mut self, checker: Box<Checker>) -> CompositeChecker {
        self.checkers.push(checker);
        self
    }
}

impl Checker for CompositeChecker {
    fn is_valid(&self, path: &Path) -> bool {
        self.checkers.iter().all(|checker| checker.is_valid(path))
    }
}
