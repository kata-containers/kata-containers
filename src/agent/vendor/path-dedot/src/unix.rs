use std::borrow::Cow;
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::{ParseDot, MAIN_SEPARATOR};

impl ParseDot for Path {
    #[inline]
    fn parse_dot(&self) -> io::Result<Cow<Path>> {
        let cwd = get_cwd!();

        self.parse_dot_from(cwd.as_ref())
    }

    fn parse_dot_from(&self, cwd: &Path) -> io::Result<Cow<Path>> {
        let mut iter = self.components();

        let mut has_dots = false;

        if let Some(first_component) = iter.next() {
            let mut tokens = Vec::new();

            let first_is_root = match first_component {
                Component::RootDir => {
                    tokens.push(MAIN_SEPARATOR.as_os_str());

                    true
                }
                Component::CurDir => {
                    for token in cwd.iter() {
                        tokens.push(token);
                    }

                    has_dots = true;

                    true
                }
                Component::ParentDir => {
                    match cwd.parent() {
                        Some(cwd_parent) => {
                            for token in cwd_parent.iter() {
                                tokens.push(token);
                            }
                        }
                        None => {
                            tokens.push(MAIN_SEPARATOR.as_os_str());
                        }
                    }

                    has_dots = true;

                    true
                }
                _ => {
                    tokens.push(first_component.as_os_str());

                    false
                }
            };

            for component in iter {
                match component {
                    Component::CurDir => {
                        // may be unreachable
                        has_dots = true;
                    }
                    Component::ParentDir => {
                        let tokens_length = tokens.len();

                        if tokens_length > 0 && (tokens_length != 1 || !first_is_root) {
                            tokens.remove(tokens_length - 1);
                        }

                        has_dots = true;
                    }
                    _ => {
                        tokens.push(component.as_os_str());
                    }
                }
            }

            let tokens_length = tokens.len();

            debug_assert!(tokens_length > 0);

            let mut size = tokens.iter().fold(tokens_length - 1, |acc, &x| acc + x.len());

            if first_is_root && tokens_length > 1 {
                size -= 1;
            }

            if has_dots || size != self.as_os_str().len() {
                let mut path_string = OsString::with_capacity(size);

                let mut iter = tokens.iter();

                path_string.push(iter.next().unwrap());

                if tokens_length > 1 {
                    if !first_is_root {
                        path_string.push(MAIN_SEPARATOR.as_os_str());
                    }

                    for token in iter.take(tokens_length - 2) {
                        path_string.push(token);

                        path_string.push(MAIN_SEPARATOR.as_os_str());
                    }

                    path_string.push(tokens[tokens_length - 1]);
                }

                let path_buf = PathBuf::from(path_string);

                Ok(Cow::from(path_buf))
            } else {
                Ok(Cow::from(self))
            }
        } else {
            Ok(Cow::from(self))
        }
    }
}
