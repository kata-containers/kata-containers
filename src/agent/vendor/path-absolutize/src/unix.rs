use std::borrow::Cow;
use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

use crate::path_dedot::{ParseDot, MAIN_SEPARATOR};
use crate::Absolutize;

impl Absolutize for Path {
    #[inline]
    fn absolutize(&self) -> io::Result<Cow<Path>> {
        let cwd = get_cwd!();

        self.absolutize_from(cwd.as_ref())
    }

    fn absolutize_from(&self, cwd: &Path) -> io::Result<Cow<'_, Path>> {
        let mut iter = self.components();

        let mut has_change = false;

        if let Some(first_component) = iter.next() {
            let mut tokens = Vec::new();

            match first_component {
                Component::RootDir => {
                    tokens.push(MAIN_SEPARATOR.as_os_str());
                }
                Component::CurDir => {
                    for token in cwd.iter() {
                        tokens.push(token);
                    }

                    has_change = true;
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

                    has_change = true;
                }
                _ => {
                    for token in cwd.iter() {
                        tokens.push(token);
                    }

                    tokens.push(first_component.as_os_str());

                    has_change = true;
                }
            }

            for component in iter {
                match component {
                    Component::CurDir => {
                        // may be unreachable
                        has_change = true;
                    }
                    Component::ParentDir => {
                        let tokens_length = tokens.len();

                        if tokens_length > 1 {
                            tokens.remove(tokens_length - 1);
                        }

                        has_change = true;
                    }
                    _ => {
                        tokens.push(component.as_os_str());
                    }
                }
            }

            let tokens_length = tokens.len();

            debug_assert!(tokens_length > 0);

            let size = tokens.iter().fold(tokens_length - 1, |acc, &x| acc + x.len()) - 1;

            if has_change || size != self.as_os_str().len() {
                let mut path_string = OsString::with_capacity(size);

                let mut iter = tokens.iter();

                path_string.push(iter.next().unwrap());

                if tokens_length > 1 {
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
            Ok(Cow::from(cwd.to_owned()))
        }
    }

    fn absolutize_virtually<P: AsRef<Path>>(&self, virtual_root: P) -> io::Result<Cow<Path>> {
        let virtual_root = virtual_root.as_ref().absolutize()?;

        let path = self.parse_dot()?;

        if path.is_absolute() {
            if !path.starts_with(&virtual_root) {
                return Err(io::Error::from(ErrorKind::InvalidInput));
            }

            Ok(path)
        } else {
            let mut virtual_root = virtual_root.into_owned();

            virtual_root.push(path);

            Ok(Cow::from(virtual_root))
        }
    }
}
