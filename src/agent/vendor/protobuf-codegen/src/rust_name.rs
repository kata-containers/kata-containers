use std::fmt;
use std::iter;

/// Valid Rust identifier
#[derive(Eq, PartialEq, Debug, Clone)]
pub(crate) struct RustIdent(String);

#[allow(dead_code)]
impl RustIdent {
    pub fn new(s: &str) -> RustIdent {
        assert!(!s.is_empty());
        assert!(!s.contains("/"), "{}", s);
        assert!(!s.contains("."), "{}", s);
        assert!(!s.contains(":"), "{}", s);
        RustIdent(s.to_owned())
    }

    pub fn super_ident() -> RustIdent {
        RustIdent::new("super")
    }

    pub fn get(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn to_path(&self) -> RustIdentWithPath {
        RustIdentWithPath::from(&self.0)
    }
}

impl fmt::Display for RustIdent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.get(), f)
    }
}

impl From<&'_ str> for RustIdent {
    fn from(s: &str) -> Self {
        RustIdent::new(s)
    }
}

impl From<String> for RustIdent {
    fn from(s: String) -> Self {
        RustIdent::new(&s)
    }
}

impl Into<String> for RustIdent {
    fn into(self) -> String {
        self.0
    }
}

#[derive(Default, Eq, PartialEq, Debug, Clone)]
pub(crate) struct RustRelativePath {
    path: Vec<RustIdent>,
}

#[allow(dead_code)]
impl RustRelativePath {
    pub fn into_path(self) -> RustPath {
        RustPath {
            absolute: false,
            path: self,
        }
    }

    pub fn empty() -> RustRelativePath {
        RustRelativePath { path: Vec::new() }
    }

    pub fn from_components<I: IntoIterator<Item = RustIdent>>(i: I) -> RustRelativePath {
        RustRelativePath {
            path: i.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    pub fn first(&self) -> Option<RustIdent> {
        self.path.iter().cloned().next()
    }

    pub fn remove_first(&mut self) -> Option<RustIdent> {
        if self.path.is_empty() {
            None
        } else {
            Some(self.path.remove(0))
        }
    }

    pub fn prepend_ident(&mut self, ident: RustIdent) {
        self.path.insert(0, ident);
    }

    pub fn append(mut self, path: RustRelativePath) -> RustRelativePath {
        for c in path.path {
            self.path.push(c);
        }
        self
    }

    pub fn push_ident(&mut self, ident: RustIdent) {
        self.path.push(ident);
    }

    pub fn _append_ident(mut self, ident: RustIdent) -> RustRelativePath {
        self.push_ident(ident);
        self
    }

    pub fn to_reverse(&self) -> RustRelativePath {
        RustRelativePath::from_components(
            iter::repeat(RustIdent::super_ident()).take(self.path.len()),
        )
    }
}

#[derive(Default, Eq, PartialEq, Debug, Clone)]
pub(crate) struct RustPath {
    absolute: bool,
    path: RustRelativePath,
}

impl fmt::Display for RustRelativePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, c) in self.path.iter().enumerate() {
            if i != 0 {
                write!(f, "::")?;
            }
            write!(f, "{}", c)?;
        }
        Ok(())
    }
}

impl From<&'_ str> for RustRelativePath {
    fn from(s: &str) -> Self {
        RustRelativePath {
            path: s.split("::").map(RustIdent::from).collect(),
        }
    }
}

#[allow(dead_code)]
impl RustPath {
    pub fn is_absolute(&self) -> bool {
        self.absolute
    }

    pub fn is_empty(&self) -> bool {
        assert!(!self.absolute);
        self.path.is_empty()
    }

    pub fn with_ident(self, ident: RustIdent) -> RustIdentWithPath {
        RustIdentWithPath { path: self, ident }
    }

    pub fn first(&self) -> Option<RustIdent> {
        assert!(!self.absolute);
        self.path.first()
    }

    pub fn remove_first(&mut self) -> Option<RustIdent> {
        assert!(!self.absolute);
        self.path.remove_first()
    }

    pub fn to_reverse(&self) -> RustPath {
        assert!(!self.absolute);
        RustPath {
            absolute: false,
            path: self.path.to_reverse(),
        }
    }

    pub fn prepend_ident(&mut self, ident: RustIdent) {
        assert!(!self.absolute);
        self.path.prepend_ident(ident);
    }

    pub fn append(self, path: RustPath) -> RustPath {
        if path.absolute {
            path
        } else {
            RustPath {
                absolute: self.absolute,
                path: self.path.append(path.path),
            }
        }
    }

    pub fn append_ident(mut self, ident: RustIdent) -> RustPath {
        self.path.path.push(ident);
        self
    }

    pub fn append_with_ident(self, path: RustIdentWithPath) -> RustIdentWithPath {
        self.append(path.path).with_ident(path.ident)
    }
}

impl From<&'_ str> for RustPath {
    fn from(s: &str) -> Self {
        let (s, absolute) = if s.starts_with("::") {
            (&s[2..], true)
        } else {
            (s, false)
        };
        RustPath {
            absolute,
            path: RustRelativePath::from(s),
        }
    }
}

impl From<String> for RustPath {
    fn from(s: String) -> Self {
        RustPath::from(&s[..])
    }
}

impl fmt::Display for RustPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.absolute {
            write!(f, "::")?;
        }
        write!(f, "{}", self.path)
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub(crate) struct RustIdentWithPath {
    pub path: RustPath,
    pub ident: RustIdent,
}

#[allow(dead_code)]
impl RustIdentWithPath {
    pub fn new(s: String) -> RustIdentWithPath {
        let mut path = RustPath::from(s);
        let ident = path.path.path.pop().unwrap();
        RustIdentWithPath { path, ident }
    }

    pub fn prepend_ident(&mut self, ident: RustIdent) {
        self.path.prepend_ident(ident)
    }

    pub fn to_path(&self) -> RustPath {
        self.path.clone().append_ident(self.ident.clone())
    }
}

impl<S: Into<String>> From<S> for RustIdentWithPath {
    fn from(s: S) -> Self {
        RustIdentWithPath::new(s.into())
    }
}

impl fmt::Display for RustIdentWithPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.to_path(), f)
    }
}
