use std::fmt;

/// Identifier in `.proto` file
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct ProtobufIdent(String);

impl ProtobufIdent {
    /// New ident from a string.
    #[allow(dead_code)]
    pub fn new(s: &str) -> ProtobufIdent {
        assert!(!s.is_empty());
        assert!(!s.contains("/"));
        assert!(!s.contains("."));
        assert!(!s.contains(":"));
        ProtobufIdent(s.to_owned())
    }

    /// Get as a string.
    pub fn get(&self) -> &str {
        &self.0
    }
}

impl From<&'_ str> for ProtobufIdent {
    fn from(s: &str) -> Self {
        ProtobufIdent::new(s)
    }
}

impl From<String> for ProtobufIdent {
    fn from(s: String) -> Self {
        ProtobufIdent::new(&s)
    }
}

impl fmt::Display for ProtobufIdent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.get(), f)
    }
}

/// Relative protobuf identifier path.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ProtobufRelativePath {
    /// The path
    pub path: String,
}

#[allow(dead_code)]
impl ProtobufRelativePath {
    /// Empty relative path.
    pub fn empty() -> ProtobufRelativePath {
        ProtobufRelativePath::new(String::new())
    }

    /// New path from a string.
    pub fn new(path: String) -> ProtobufRelativePath {
        assert!(!path.starts_with("."));

        ProtobufRelativePath { path }
    }

    /// From path components.
    pub fn from_components<I: IntoIterator<Item = ProtobufIdent>>(i: I) -> ProtobufRelativePath {
        let v: Vec<String> = i.into_iter().map(|c| c.get().to_owned()).collect();
        ProtobufRelativePath::from(v.join("."))
    }

    /// Get the string.
    pub fn get(&self) -> &str {
        &self.path
    }

    /// The path is empty.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// As absolute path from root namespace.
    pub fn into_absolute(self) -> ProtobufAbsolutePath {
        if self.is_empty() {
            ProtobufAbsolutePath::root()
        } else {
            ProtobufAbsolutePath::from(format!(".{}", self))
        }
    }

    fn _last_part(&self) -> Option<&str> {
        match self.path.rfind('.') {
            Some(pos) => Some(&self.path[pos + 1..]),
            None => {
                if self.path.is_empty() {
                    None
                } else {
                    Some(&self.path)
                }
            }
        }
    }

    fn parent(&self) -> Option<ProtobufRelativePath> {
        match self.path.rfind('.') {
            Some(pos) => Some(ProtobufRelativePath::new(self.path[..pos].to_owned())),
            None => {
                if self.path.is_empty() {
                    None
                } else {
                    Some(ProtobufRelativePath::empty())
                }
            }
        }
    }

    /// Self path and parent paths.
    pub fn self_and_parents(&self) -> Vec<ProtobufRelativePath> {
        let mut tmp = self.clone();

        let mut r = Vec::new();

        r.push(self.clone());

        while let Some(parent) = tmp.parent() {
            r.push(parent.clone());
            tmp = parent;
        }

        r
    }

    /// Append path component.
    pub fn append(&self, simple: &ProtobufRelativePath) -> ProtobufRelativePath {
        if self.path.is_empty() {
            ProtobufRelativePath::from(simple.get())
        } else {
            ProtobufRelativePath::new(format!("{}.{}", self.path, simple))
        }
    }

    /// Append identifier to the path.
    pub fn append_ident(&self, simple: &ProtobufIdent) -> ProtobufRelativePath {
        self.append(&ProtobufRelativePath::from(simple.clone()))
    }

    /// Get first component path and remaining.
    pub fn split_first_rem(&self) -> Option<(ProtobufIdent, ProtobufRelativePath)> {
        if self.is_empty() {
            None
        } else {
            Some(match self.path.find('.') {
                Some(dot) => (
                    ProtobufIdent::from(&self.path[..dot]),
                    ProtobufRelativePath::new(self.path[dot + 1..].to_owned()),
                ),
                None => (
                    ProtobufIdent::from(self.path.clone()),
                    ProtobufRelativePath::empty(),
                ),
            })
        }
    }
}

impl From<&'_ str> for ProtobufRelativePath {
    fn from(s: &str) -> ProtobufRelativePath {
        ProtobufRelativePath::from(s.to_owned())
    }
}

impl From<String> for ProtobufRelativePath {
    fn from(s: String) -> ProtobufRelativePath {
        ProtobufRelativePath::new(s)
    }
}

impl From<ProtobufIdent> for ProtobufRelativePath {
    fn from(s: ProtobufIdent) -> ProtobufRelativePath {
        ProtobufRelativePath::from(s.get())
    }
}

impl From<Vec<ProtobufIdent>> for ProtobufRelativePath {
    fn from(s: Vec<ProtobufIdent>) -> ProtobufRelativePath {
        ProtobufRelativePath::from_components(s.into_iter())
    }
}

impl fmt::Display for ProtobufRelativePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.path, f)
    }
}

#[cfg(test)]
mod relative_path_test {
    use super::*;

    #[test]
    fn parent() {
        assert_eq!(None, ProtobufRelativePath::empty().parent());
        assert_eq!(
            Some(ProtobufRelativePath::empty()),
            ProtobufRelativePath::new("aaa".to_owned()).parent()
        );
        assert_eq!(
            Some(ProtobufRelativePath::new("abc".to_owned())),
            ProtobufRelativePath::new("abc.def".to_owned()).parent()
        );
        assert_eq!(
            Some(ProtobufRelativePath::new("abc.def".to_owned())),
            ProtobufRelativePath::new("abc.def.gh".to_owned()).parent()
        );
    }

    #[test]
    fn last_part() {
        assert_eq!(None, ProtobufRelativePath::empty()._last_part());
        assert_eq!(
            Some("aaa"),
            ProtobufRelativePath::new("aaa".to_owned())._last_part()
        );
        assert_eq!(
            Some("def"),
            ProtobufRelativePath::new("abc.def".to_owned())._last_part()
        );
        assert_eq!(
            Some("gh"),
            ProtobufRelativePath::new("abc.def.gh".to_owned())._last_part()
        );
    }
}

/// Absolute protobuf path (e. g. package).
///
/// This is not filesystem path.
#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub struct ProtobufAbsolutePath {
    /// The path.
    pub path: String,
}

impl ProtobufAbsolutePath {
    fn root() -> ProtobufAbsolutePath {
        ProtobufAbsolutePath::new(String::new())
    }

    /// From string.
    pub fn new(path: String) -> ProtobufAbsolutePath {
        assert!(path.is_empty() || path.starts_with("."), "{}", path);
        assert!(!path.ends_with("."), "{}", path);
        ProtobufAbsolutePath { path }
    }

    /// The path is empty.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// From a path without leading dot.
    ///
    /// (Protobuf paths start with dot).
    pub fn from_path_without_dot(path: &str) -> ProtobufAbsolutePath {
        if path.is_empty() {
            ProtobufAbsolutePath::root()
        } else {
            assert!(!path.starts_with("."));
            assert!(!path.ends_with("."));
            ProtobufAbsolutePath::new(format!(".{}", path))
        }
    }

    /// Parse absolute path.
    #[allow(dead_code)]
    pub fn from_package_path(path: Option<&str>) -> ProtobufAbsolutePath {
        match path {
            None => ProtobufAbsolutePath::root(),
            Some(path) => ProtobufAbsolutePath::from_path_without_dot(path),
        }
    }

    /// Construct abs path from a string which may start with a dot.
    pub fn from_path_maybe_dot(path: &str) -> ProtobufAbsolutePath {
        if path.starts_with(".") {
            ProtobufAbsolutePath::new(path.to_owned())
        } else {
            ProtobufAbsolutePath::from_path_without_dot(path)
        }
    }

    /// Push identifier to the path.
    pub fn push_simple(&mut self, simple: ProtobufIdent) {
        self.path.push('.');
        self.path.push_str(simple.get());
    }

    /// Push relative path.
    pub fn push_relative(&mut self, relative: &ProtobufRelativePath) {
        if !relative.is_empty() {
            self.path.push('.');
            self.path.push_str(&relative.path);
        }
    }

    /// Try remove a prefix.
    pub fn remove_prefix(&self, prefix: &ProtobufAbsolutePath) -> Option<ProtobufRelativePath> {
        if self.path.starts_with(&prefix.path) {
            let rem = &self.path[prefix.path.len()..];
            if rem.is_empty() {
                return Some(ProtobufRelativePath::empty());
            }
            if rem.starts_with('.') {
                return Some(ProtobufRelativePath::new(rem[1..].to_owned()));
            }
        }
        None
    }
}

impl From<&'_ str> for ProtobufAbsolutePath {
    fn from(s: &str) -> Self {
        ProtobufAbsolutePath::new(s.to_owned())
    }
}

impl From<String> for ProtobufAbsolutePath {
    fn from(s: String) -> Self {
        ProtobufAbsolutePath::new(s)
    }
}

impl fmt::Display for ProtobufAbsolutePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.path, f)
    }
}

#[cfg(test)]
mod absolute_path_test {
    use super::*;

    #[test]
    fn absolute_path_push_simple() {
        let mut foo = ProtobufAbsolutePath::new(".foo".to_owned());
        foo.push_simple(ProtobufIdent::from("bar"));
        assert_eq!(ProtobufAbsolutePath::new(".foo.bar".to_owned()), foo);

        let mut foo = ProtobufAbsolutePath::root();
        foo.push_simple(ProtobufIdent::from("bar"));
        assert_eq!(ProtobufAbsolutePath::new(".bar".to_owned()), foo);
    }

    #[test]
    fn absolute_path_remove_prefix() {
        assert_eq!(
            Some(ProtobufRelativePath::empty()),
            ProtobufAbsolutePath::new(".foo".to_owned())
                .remove_prefix(&ProtobufAbsolutePath::new(".foo".to_owned()))
        );
        assert_eq!(
            Some(ProtobufRelativePath::new("bar".to_owned())),
            ProtobufAbsolutePath::new(".foo.bar".to_owned())
                .remove_prefix(&ProtobufAbsolutePath::new(".foo".to_owned()))
        );
        assert_eq!(
            Some(ProtobufRelativePath::new("baz.qux".to_owned())),
            ProtobufAbsolutePath::new(".foo.bar.baz.qux".to_owned())
                .remove_prefix(&ProtobufAbsolutePath::new(".foo.bar".to_owned()))
        );
        assert_eq!(
            None,
            ProtobufAbsolutePath::new(".foo.barbaz".to_owned())
                .remove_prefix(&ProtobufAbsolutePath::new(".foo.bar".to_owned()))
        );
    }
}
