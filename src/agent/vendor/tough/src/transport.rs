#[cfg(feature = "http")]
use crate::{HttpTransport, HttpTransportBuilder};
use dyn_clone::DynClone;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::io::{ErrorKind, Read};
use std::path::PathBuf;
use url::Url;

/// A trait to abstract over the method/protocol by which files are obtained.
///
/// The trait hides the underlying types involved by returning the `Read` object as a
/// `Box<dyn Read + Send>` and by requiring concrete type [`TransportError`] as the error type.
///
/// Inclusion of the `DynClone` trait means that you will need to implement `Clone` when
/// implementing a `Transport`.
pub trait Transport: Debug + DynClone {
    /// Opens a `Read` object for the file specified by `url`.
    fn fetch(&self, url: Url) -> Result<Box<dyn Read + Send>, TransportError>;
}

// Implements `Clone` for `Transport` trait objects (i.e. on `Box::<dyn Clone>`). To facilitate
// this, `Clone` needs to be implemented for any `Transport`s. The compiler will enforce this.
dyn_clone::clone_trait_object!(Transport);

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// The kind of error that the transport object experienced during `fetch`.
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum TransportErrorKind {
    /// The [`Transport`] does not handle the URL scheme. e.g. `file://` or `http://`.
    UnsupportedUrlScheme,
    /// The file cannot be found.
    ///
    /// Some TUF operations could benefit from knowing whether a [`Transport`] failure is a result
    /// of a file not existing. In particular:
    /// > TUF v1.0.16 5.2.2. Try downloading version N+1 of the root metadata file `[...]` If this
    /// > file is not available `[...]` then go to step 5.1.9.
    ///
    /// We want to distinguish cases when a specific file probably doesn't exist from cases where
    /// the failure to fetch it is due to some other problem (i.e. some fault in the [`Transport`]
    /// or the machine hosting the file).
    ///
    /// For some transports, the distinction is obvious. For example, a local file transport should
    /// return `FileNotFound` for `std::error::ErrorKind::NotFound` and nothing else. For other
    /// transports it might be less obvious, but the intent of `FileNotFound` is to indicate that
    /// the file probably doesn't exist.
    FileNotFound,
    /// The transport failed for any other reason, e.g. IO error, HTTP broken pipe, etc.
    Other,
}

impl Display for TransportErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TransportErrorKind::UnsupportedUrlScheme => "unsupported URL scheme",
                TransportErrorKind::FileNotFound => "file not found",
                TransportErrorKind::Other => "other",
            }
        )
    }
}

/// The error type that [`Transport::fetch`] returns.
#[derive(Debug)]
pub struct TransportError {
    /// The kind of error that occurred.
    kind: TransportErrorKind,
    /// The URL that the transport was trying to fetch.
    url: String,
    /// The underlying error that occurred (if any).
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl TransportError {
    /// Creates a new [`TransportError`]. Use this when there is no underlying error to wrap.
    pub fn new<S>(kind: TransportErrorKind, url: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            kind,
            url: url.as_ref().into(),
            source: None,
        }
    }

    /// Creates a new [`TransportError`]. Use this to preserve an underlying error.
    pub fn new_with_cause<S, E>(kind: TransportErrorKind, url: S, source: E) -> Self
    where
        E: Into<Box<dyn Error + Send + Sync>>,
        S: AsRef<str>,
    {
        Self {
            kind,
            url: url.as_ref().into(),
            source: Some(source.into()),
        }
    }

    /// The type of [`Transport`] error that occurred.
    pub fn kind(&self) -> TransportErrorKind {
        self.kind
    }

    /// The URL that the [`Transport`] was trying to fetch when the error occurred.
    pub fn url(&self) -> &str {
        self.url.as_str()
    }
}

impl Display for TransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(e) = self.source.as_ref() {
            write!(
                f,
                "Transport '{}' error fetching '{}': {}",
                self.kind, self.url, e
            )
        } else {
            write!(f, "Transport '{}' error fetching '{}'", self.kind, self.url)
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &(dyn Error))
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// Provides a [`Transport`] for local files.
#[derive(Debug, Clone, Copy)]
pub struct FilesystemTransport;

impl Transport for FilesystemTransport {
    fn fetch(&self, url: Url) -> Result<Box<dyn Read + Send>, TransportError> {
        // If the scheme isn't "file://", reject
        if url.scheme() != "file" {
            return Err(TransportError::new(
                TransportErrorKind::UnsupportedUrlScheme,
                url,
            ));
        }

        // Convert the file URL into a file path. We need to use url.path() and not
        // url.to_file_path() because to_file_path will decode the percent encoding which could
        // restore path traversal characters.
        let file_path = PathBuf::from(url.path());

        // And open the file
        let f = std::fs::File::open(file_path).map_err(|e| {
            let kind = match e.kind() {
                ErrorKind::NotFound => TransportErrorKind::FileNotFound,
                _ => TransportErrorKind::Other,
            };
            TransportError::new_with_cause(kind, url, e)
        })?;
        Ok(Box::new(f))
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// A Transport that provides support for both local files and, if the `http` feature is enabled,
/// HTTP-transported files.
#[derive(Debug, Clone, Copy)]
pub struct DefaultTransport {
    file: FilesystemTransport,
    #[cfg(feature = "http")]
    http: HttpTransport,
}

impl Default for DefaultTransport {
    fn default() -> Self {
        Self {
            file: FilesystemTransport,
            #[cfg(feature = "http")]
            http: HttpTransport::default(),
        }
    }
}

impl DefaultTransport {
    /// Creates a new `DefaultTransport`. Same as `default()`.
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "http")]
impl DefaultTransport {
    /// Create a new `DefaultTransport` with potentially customized settings.
    pub fn new_with_http_settings(builder: HttpTransportBuilder) -> Self {
        Self {
            file: FilesystemTransport,
            http: builder.build(),
        }
    }
}

impl Transport for DefaultTransport {
    fn fetch(&self, url: Url) -> Result<Box<dyn Read + Send>, TransportError> {
        match url.scheme() {
            "file" => self.file.fetch(url),
            "http" | "https" => self.handle_http(url),
            _ => Err(TransportError::new(
                TransportErrorKind::UnsupportedUrlScheme,
                url,
            )),
        }
    }
}

impl DefaultTransport {
    #[cfg(not(feature = "http"))]
    #[allow(clippy::trivially_copy_pass_by_ref, clippy::unused_self)]
    fn handle_http(&self, url: Url) -> Result<Box<dyn Read + Send>, TransportError> {
        Err(TransportError::new_with_cause(
            TransportErrorKind::UnsupportedUrlScheme,
            url,
            "The library was not compiled with the http feature enabled.",
        ))
    }

    #[cfg(feature = "http")]
    fn handle_http(&self, url: Url) -> Result<Box<dyn Read + Send>, TransportError> {
        self.http.fetch(url)
    }
}
