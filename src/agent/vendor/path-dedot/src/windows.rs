#![cfg(windows)]

use super::{ParseDot, CWD, MAIN_SEPARATOR};

use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf, PrefixComponent};

pub trait ParsePrefix {
    fn get_path_prefix(&self) -> Option<PrefixComponent>;
}

impl ParsePrefix for Path {
    #[inline]
    fn get_path_prefix(&self) -> Option<PrefixComponent> {
        let first_component = self.components().next();

        match first_component.unwrap() {
            Component::Prefix(prefix_component) => Some(prefix_component),
            _ => None,
        }
    }
}

impl ParsePrefix for PathBuf {
    #[inline]
    fn get_path_prefix(&self) -> Option<PrefixComponent> {
        let first_component = self.components().next();

        match first_component.unwrap() {
            Component::Prefix(prefix_component) => Some(prefix_component),
            _ => None,
        }
    }
}

impl ParseDot for Path {
    fn parse_dot(&self) -> io::Result<PathBuf> {
        let mut size = self.as_os_str().len();

        let cwd = unsafe { CWD.initial() };

        let mut tokens = Vec::new();

        let mut iter = self.iter();

        let mut prefix = self.get_path_prefix();

        if let Some(first_token) = iter.next() {
            if first_token.eq(".") {
                prefix = cwd.get_path_prefix();

                for token in cwd.iter() {
                    tokens.push(token);
                }
                size += cwd.as_os_str().len() - 1;
            } else if first_token.eq("..") {
                prefix = cwd.get_path_prefix();

                let cwd_parent = cwd.parent();

                match cwd_parent {
                    Some(cwd_parent) => {
                        for token in cwd_parent.iter() {
                            tokens.push(token);
                        }
                        size += cwd_parent.as_os_str().len() - 2;
                    }
                    None => {
                        let prefix = prefix.unwrap().as_os_str();
                        tokens.push(prefix);
                        tokens.push(MAIN_SEPARATOR.as_os_str());
                        size -= 2;
                    }
                }
            } else {
                tokens.push(first_token);

                if let Some(prefix) = prefix {
                    // single dot is filtered by the iterator
                    let path = self.as_os_str().to_str().unwrap();

                    let prefix_len = prefix.as_os_str().len();
                    let path_len = path.len();

                    if prefix_len < path_len
                        && path[prefix_len..prefix_len + 1].eq(".")
                        && (prefix_len + 1 == path_len
                            || path[prefix_len + 1..prefix_len + 2].eq(r"\"))
                    {
                        for token in cwd.iter().skip(1) {
                            tokens.push(token);
                        }
                        size += cwd.as_os_str().len() - 1;
                        size -= cwd.get_path_prefix().unwrap().as_os_str().len();
                    } else if let Some(second_token) = iter.next() {
                        if second_token.eq("..") {
                            let cwd_parent = cwd.parent();

                            match cwd_parent {
                                Some(cwd_parent) => {
                                    for token in cwd_parent.iter().skip(1) {
                                        tokens.push(token);
                                    }
                                    size += cwd_parent.as_os_str().len() - 1;
                                    size -= cwd.get_path_prefix().unwrap().as_os_str().len();
                                }
                                None => {
                                    tokens.push(MAIN_SEPARATOR.as_os_str());
                                    size -= 2;
                                }
                            }
                        } else {
                            tokens.push(second_token);
                        }
                    }
                }
            }

            if prefix.is_some() {
                for token in iter {
                    //                  if token.eq(".") {
                    //                      size -= 2;
                    //                      continue;
                    //                  } else
                    // Don't need to check single dot. It is already filtered.
                    if token.eq("..") {
                        let len = tokens.len();

                        if len > 1 && (len != 2 || tokens[1].ne(MAIN_SEPARATOR.as_os_str())) {
                            let removed = tokens.remove(len - 1);
                            size -= removed.len() + 4;
                        } else {
                            size -= 3;
                        }
                    } else {
                        tokens.push(token);
                    }
                }
            } else {
                for token in iter {
                    //                  if token.eq(".") {
                    //                      size -= 2;
                    //                      continue;
                    //                  } else
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
        }

        let mut path = OsString::with_capacity(size);

        let len = tokens.len();

        if len > 0 {
            let mut iter = tokens.iter();

            if let Some(first_token) = iter.next() {
                path.push(first_token);

                if len > 1 {
                    if prefix.is_some() {
                        let second_token = iter.next().unwrap();

                        path.push(second_token);

                        if !second_token.eq(&MAIN_SEPARATOR.as_os_str()) {
                            path.push(MAIN_SEPARATOR.as_os_str());
                        }

                        if len > 2 {
                            for &token in iter.take(len - 3) {
                                path.push(token);

                                path.push(MAIN_SEPARATOR.as_os_str());
                            }

                            path.push(tokens[len - 1]);
                        }
                    } else {
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
        }

        let path_buf = PathBuf::from(path);

        Ok(path_buf)
    }
}
