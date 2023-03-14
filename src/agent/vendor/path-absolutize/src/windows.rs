use std::borrow::Cow;
use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

use crate::path_dedot::{ParseDot, ParsePrefix, MAIN_SEPARATOR};
use crate::Absolutize;

impl Absolutize for Path {
    #[inline]
    fn absolutize(&self) -> io::Result<Cow<Path>> {
        let cwd = get_cwd!();

        self.absolutize_from(&cwd)
    }

    fn absolutize_from(&self, cwd: &Path) -> io::Result<Cow<'_, Path>> {
        let mut iter = self.components();

        let mut has_change = false;

        if let Some(first_component) = iter.next() {
            let mut tokens = Vec::new();

            match first_component {
                Component::Prefix(prefix) => {
                    tokens.push(prefix.as_os_str());

                    if let Some(second_component) = iter.next() {
                        match second_component {
                            Component::RootDir => {
                                tokens.push(MAIN_SEPARATOR.as_os_str());
                            }
                            Component::CurDir => {
                                // may be unreachable

                                for token in cwd.iter().skip(1) {
                                    tokens.push(token);
                                }

                                has_change = true;
                            }
                            Component::ParentDir => {
                                match cwd.parent() {
                                    Some(cwd_parent) => {
                                        for token in cwd_parent.iter().skip(1) {
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
                                for token in cwd.iter().skip(1) {
                                    tokens.push(token);
                                }

                                tokens.push(second_component.as_os_str());

                                has_change = true;
                            }
                        }
                    } else {
                        tokens.push(MAIN_SEPARATOR.as_os_str());

                        has_change = true;
                    }
                }
                Component::RootDir => {
                    let prefix = cwd.get_path_prefix().unwrap().as_os_str();
                    tokens.push(prefix);

                    tokens.push(MAIN_SEPARATOR.as_os_str());

                    has_change = true;
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
                            let prefix = cwd.get_path_prefix().unwrap().as_os_str();
                            tokens.push(prefix);

                            tokens.push(MAIN_SEPARATOR.as_os_str());
                        }
                    }

                    has_change = true;
                }
                Component::Normal(token) => {
                    for token in cwd.iter() {
                        tokens.push(token);
                    }

                    tokens.push(token);

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

                        if tokens_length > 2 {
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

            let mut size = tokens.iter().fold(tokens_length - 1, |acc, &x| acc + x.len()) - 1;

            if tokens_length > 2 {
                size -= 1;
            }

            if has_change || size != self.as_os_str().len() {
                let mut path_string = OsString::with_capacity(size);

                let mut iter = tokens.iter();

                path_string.push(iter.next().unwrap());
                path_string.push(iter.next().unwrap());

                if tokens_length > 2 {
                    for token in iter.take(tokens_length - 3) {
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
            let path_lowercase = path
                .to_str()
                .ok_or_else(|| io::Error::new(ErrorKind::Other, "The path is not valid UTF-8."))?
                .to_lowercase();

            let virtual_root_lowercase = virtual_root
                .to_str()
                .ok_or_else(|| {
                    io::Error::new(ErrorKind::Other, "The virtual root is not valid UTF-8.")
                })?
                .to_lowercase();

            if !&path_lowercase.starts_with(&virtual_root_lowercase) {
                return Err(io::Error::from(ErrorKind::InvalidInput));
            }

            Ok(path)
        } else if let Some(prefix) = path.get_path_prefix() {
            let prefix = prefix.as_os_str().to_str().ok_or_else(|| {
                io::Error::new(ErrorKind::Other, "The prefix of the path is not valid UTF-8.")
            })?;

            let prefix_lowercase = prefix.to_lowercase();

            let virtual_root_prefix_lowercase = virtual_root
                .get_path_prefix()
                .unwrap()
                .as_os_str()
                .to_str()
                .ok_or_else(|| {
                    io::Error::new(
                        ErrorKind::Other,
                        "The prefix of the virtual root is not valid UTF-8.",
                    )
                })?
                .to_lowercase();

            if prefix_lowercase == virtual_root_prefix_lowercase {
                let path = path.to_str().ok_or_else(|| {
                    io::Error::new(ErrorKind::Other, "The path is not valid UTF-8.")
                })?;

                let path_without_prefix = Path::new(&path[prefix.len()..]);

                let mut virtual_root = virtual_root.into_owned();

                virtual_root.push(path_without_prefix);

                Ok(Cow::from(virtual_root))
            } else {
                Err(io::Error::from(ErrorKind::InvalidInput))
            }
        } else {
            let mut virtual_root = virtual_root.into_owned();

            virtual_root.push(path);

            Ok(Cow::from(virtual_root))
        }
    }
}
