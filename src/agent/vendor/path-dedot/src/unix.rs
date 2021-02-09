#![cfg(not(windows))]

use super::{ParseDot, CWD, MAIN_SEPARATOR};

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

impl ParseDot for Path {
    fn parse_dot(&self) -> io::Result<PathBuf> {
        let mut size = self.as_os_str().len();

        let cwd = unsafe { CWD.initial() };

        let mut tokens = Vec::new();

        let mut iter = self.iter();

        if let Some(first_token) = iter.next() {
            if first_token.eq(".") {
                for token in cwd.iter() {
                    tokens.push(token);
                }
                size += cwd.as_os_str().len() - 1;
            } else if first_token.eq("..") {
                let cwd_parent = cwd.parent();

                match cwd_parent {
                    Some(cwd_parent) => {
                        for token in cwd_parent.iter() {
                            tokens.push(token);
                        }
                        size += cwd_parent.as_os_str().len() - 2;
                    }
                    None => {
                        tokens.push(MAIN_SEPARATOR.as_os_str());
                        size -= 2;
                    }
                }
            } else {
                tokens.push(first_token);
            }

            for token in iter {
                //              if token.eq(".") {
                //                  size -= 2;
                //                  continue;
                //              } else
                // Don't need to check single dot. It is already filtered.
                if token.eq("..") {
                    let len = tokens.len();
                    if len > 0 && (len != 1 || tokens[0].ne(MAIN_SEPARATOR.as_os_str())) {
                        let removed = tokens.remove(len - 1);
                        size -= removed.len() + 4;
                    } else {
                        size -= 3;
                    }
                } else {
                    tokens.push(token);
                }
            }
        }

        let mut path = OsString::with_capacity(size);

        let len = tokens.len();

        if len > 0 {
            let mut iter = tokens.iter();

            if let Some(first_token) = iter.next() {
                path.push(first_token);

                if len > 1 {
                    if !first_token.eq(&MAIN_SEPARATOR.as_os_str()) {
                        path.push(MAIN_SEPARATOR.as_os_str());
                    }

                    for &token in iter.take(len - 2) {
                        path.push(token);

                        path.push(MAIN_SEPARATOR.as_os_str());
                    }

                    path.push(tokens[len - 1]);
                }
            }
        }

        let path_buf = PathBuf::from(path);

        Ok(path_buf)
    }
}
