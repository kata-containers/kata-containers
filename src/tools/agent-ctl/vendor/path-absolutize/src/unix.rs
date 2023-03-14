#![cfg(not(windows))]

use super::Absolutize;

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use path_dedot::{ParseDot, CWD};

impl Absolutize for Path {
    fn absolutize(&self) -> io::Result<PathBuf> {
        if self.is_absolute() {
            self.parse_dot()
        } else {
            let cwd = unsafe {
                CWD.initial()
            };

            let path = Path::join(cwd.as_path(), self);

            path.parse_dot()
        }
    }

    fn absolutize_virtually<P: AsRef<Path>>(&self, virtual_root: P) -> io::Result<PathBuf> {
        let mut virtual_root = virtual_root.as_ref().absolutize()?;

        if self.is_absolute() {
            let path = self.parse_dot()?;

            if !path.starts_with(&virtual_root) {
                return Err(io::Error::from(ErrorKind::InvalidInput));
            }

            Ok(path)
        } else {
            let path = self.parse_dot()?;

            if path.is_absolute() {
                if !path.starts_with(&virtual_root) {
                    return Err(io::Error::from(ErrorKind::InvalidInput));
                }

                Ok(path)
            } else {
                virtual_root.push(path);

                Ok(virtual_root)
            }
        }
    }
}
