// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Error, Formatter};
use std::str::Utf8Error;

pub mod headers;
pub mod sock_ctrl_msg;

pub mod ascii {
    pub const CR: u8 = b'\r';
    pub const COLON: u8 = b':';
    pub const LF: u8 = b'\n';
    pub const SP: u8 = b' ';
    pub const CRLF_LEN: usize = 2;
}

///Errors associated with a header that is invalid.
#[derive(Debug, Eq, PartialEq)]
pub enum HttpHeaderError {
    /// The header is misformatted.
    InvalidFormat(String),
    /// The specified header contains illegal characters.
    InvalidUtf8String(Utf8Error),
    ///The value specified is not valid.
    InvalidValue(String, String),
    /// The content length specified is longer than the limit imposed by Micro Http.
    SizeLimitExceeded(String),
    /// The requested feature is not currently supported.
    UnsupportedFeature(String, String),
    /// The header specified is not supported.
    UnsupportedName(String),
    /// The value for the specified header is not supported.
    UnsupportedValue(String, String),
}

impl Display for HttpHeaderError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::InvalidFormat(header_key) => {
                write!(f, "Header is incorrectly formatted. Key: {}", header_key)
            }
            Self::InvalidUtf8String(header_key) => {
                write!(f, "Header contains invalid characters. Key: {}", header_key)
            }
            Self::InvalidValue(header_name, value) => {
                write!(f, "Invalid value. Key:{}; Value:{}", header_name, value)
            }
            Self::SizeLimitExceeded(inner) => {
                write!(f, "Invalid content length. Header: {}", inner)
            }
            Self::UnsupportedFeature(header_key, header_value) => write!(
                f,
                "Unsupported feature. Key: {}; Value: {}",
                header_key, header_value
            ),
            Self::UnsupportedName(inner) => write!(f, "Unsupported header name. Key: {}", inner),
            Self::UnsupportedValue(header_key, header_value) => write!(
                f,
                "Unsupported value. Key:{}; Value:{}",
                header_key, header_value
            ),
        }
    }
}

/// Errors associated with parsing the HTTP Request from a u8 slice.
#[derive(Debug, Eq, PartialEq)]
pub enum RequestError {
    /// No request was pending while the request body was being parsed.
    BodyWithoutPendingRequest,
    /// Header specified is either invalid or not supported by this HTTP implementation.
    HeaderError(HttpHeaderError),
    /// No request was pending while the request headers were being parsed.
    HeadersWithoutPendingRequest,
    /// The HTTP Method is not supported or it is invalid.
    InvalidHttpMethod(&'static str),
    /// The HTTP Version in the Request is not supported or it is invalid.
    InvalidHttpVersion(&'static str),
    /// The Request is invalid and cannot be served.
    InvalidRequest,
    /// Request URI is invalid.
    InvalidUri(&'static str),
    /// Overflow occurred when parsing a request.
    Overflow,
    /// Underflow occurred when parsing a request.
    Underflow,
    /// Payload too large.
    SizeLimitExceeded(usize, usize),
}

impl Display for RequestError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::BodyWithoutPendingRequest => write!(
                f,
                "No request was pending while the request body was being parsed."
            ),
            Self::HeaderError(inner) => write!(f, "Invalid header. Reason: {}", inner),
            Self::HeadersWithoutPendingRequest => write!(
                f,
                "No request was pending while the request headers were being parsed."
            ),
            Self::InvalidHttpMethod(inner) => write!(f, "Invalid HTTP Method: {}", inner),
            Self::InvalidHttpVersion(inner) => write!(f, "Invalid HTTP Version: {}", inner),
            Self::InvalidRequest => write!(f, "Invalid request."),
            Self::InvalidUri(inner) => write!(f, "Invalid URI: {}", inner),
            Self::Overflow => write!(f, "Overflow occurred when parsing a request."),
            Self::Underflow => write!(f, "Underflow occurred when parsing a request."),
            Self::SizeLimitExceeded(limit, size) => write!(
                f,
                "Request payload with size {} is larger than the limit of {} \
                 allowed by server.",
                size, limit
            ),
        }
    }
}

/// Errors associated with a HTTP Connection.
#[derive(Debug)]
pub enum ConnectionError {
    /// Attempted to read or write on a closed connection.
    ConnectionClosed,
    /// Attempted to write on a stream when there was nothing to write.
    InvalidWrite,
    /// The request parsing has failed.
    ParseError(RequestError),
    /// Could not perform a read operation from stream successfully.
    StreamReadError(SysError),
    /// Could not perform a write operation to stream successfully.
    StreamWriteError(std::io::Error),
}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::ConnectionClosed => write!(f, "Connection closed."),
            Self::InvalidWrite => write!(f, "Invalid write attempt."),
            Self::ParseError(inner) => write!(f, "Parsing error: {}", inner),
            Self::StreamReadError(inner) => write!(f, "Reading stream error: {}", inner),
            // Self::StreamReadError2(inner) => write!(f, "Reading stream error: {}", inner),
            Self::StreamWriteError(inner) => write!(f, "Writing stream error: {}", inner),
        }
    }
}

/// Errors pertaining to `HttpRoute`.
#[derive(Debug)]
#[allow(dead_code)]
pub enum RouteError {
    /// Handler for http routing path already exists.
    HandlerExist(String),
}

impl Display for RouteError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            RouteError::HandlerExist(p) => write!(f, "handler for {} already exists", p),
        }
    }
}

/// Errors pertaining to `HttpServer`.
#[derive(Debug)]
pub enum ServerError {
    /// Error from one of the connections.
    ConnectionError(ConnectionError),
    /// Epoll operations failed.
    IOError(std::io::Error),
    /// Overflow occured while processing messages.
    Overflow,
    /// Server maximum capacity has been reached.
    ServerFull,
    /// Underflow occured while processing mesagges.
    Underflow,
}

impl Display for ServerError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::ConnectionError(inner) => write!(f, "Connection error: {}", inner),
            Self::IOError(inner) => write!(f, "IO error: {}", inner),
            Self::Overflow => write!(f, "Overflow occured while processing messages."),
            Self::ServerFull => write!(f, "Server is full."),
            Self::Underflow => write!(f, "Underflow occured while processing messages."),
        }
    }
}

/// The Body associated with an HTTP Request or Response.
///
/// ## Examples
/// ```
/// use dbs_uhttp::Body;
/// let body = Body::new("This is a test body.".to_string());
/// assert_eq!(body.raw(), b"This is a test body.");
/// assert_eq!(body.len(), 20);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Body {
    /// Body of the HTTP message as bytes.
    pub body: Vec<u8>,
}

impl Body {
    /// Creates a new `Body` from a `String` input.
    pub fn new<T: Into<Vec<u8>>>(body: T) -> Self {
        Self { body: body.into() }
    }

    /// Returns the body as an `u8 slice`.
    pub fn raw(&self) -> &[u8] {
        self.body.as_slice()
    }

    /// Returns the length of the `Body`.
    pub fn len(&self) -> usize {
        self.body.len()
    }

    /// Checks if the body is empty, ie with zero length
    pub fn is_empty(&self) -> bool {
        self.body.len() == 0
    }
}

/// Supported HTTP Methods.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Method {
    /// GET Method.
    Get,
    /// HEAD Method.
    Head,
    /// POST Method.
    Post,
    /// PUT Method.
    Put,
    /// PATCH Method.
    Patch,
    /// Delete Method.
    Delete,
}

impl Method {
    /// Returns a `Method` object if the parsing of `bytes` is successful.
    ///
    /// The method is case sensitive. A call to try_from with the input b"get" will return
    /// an error, but when using the input b"GET", it returns Method::Get.
    ///
    /// # Errors
    /// `InvalidHttpMethod` is returned if the specified HTTP method is unsupported.
    pub fn try_from(bytes: &[u8]) -> Result<Self, RequestError> {
        match bytes {
            b"GET" => Ok(Self::Get),
            b"HEAD" => Ok(Self::Head),
            b"POST" => Ok(Self::Post),
            b"PUT" => Ok(Self::Put),
            b"PATCH" => Ok(Self::Patch),
            b"DELETE" => Ok(Self::Delete),
            _ => Err(RequestError::InvalidHttpMethod("Unsupported HTTP method.")),
        }
    }

    /// Returns an `u8 slice` corresponding to the Method.
    pub fn raw(self) -> &'static [u8] {
        match self {
            Self::Get => b"GET",
            Self::Head => b"HEAD",
            Self::Post => b"POST",
            Self::Put => b"PUT",
            Self::Patch => b"PATCH",
            Self::Delete => b"DELETE",
        }
    }

    /// Returns an &str corresponding to the Method.
    pub fn to_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

/// Supported HTTP Versions.
///
/// # Examples
/// ```
/// use dbs_uhttp::Version;
/// let version = Version::try_from(b"HTTP/1.1");
/// assert!(version.is_ok());
///
/// let version = Version::try_from(b"http/1.1");
/// assert!(version.is_err());
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Version {
    /// HTTP/1.0
    Http10,
    /// HTTP/1.1
    Http11,
}

impl Default for Version {
    /// Returns the default HTTP version = HTTP/1.1.
    fn default() -> Self {
        Self::Http11
    }
}

impl Version {
    /// HTTP Version as an `u8 slice`.
    pub fn raw(self) -> &'static [u8] {
        match self {
            Self::Http10 => b"HTTP/1.0",
            Self::Http11 => b"HTTP/1.1",
        }
    }

    /// Creates a new HTTP Version from an `u8 slice`.
    ///
    /// The supported versions are HTTP/1.0 and HTTP/1.1.
    /// The version is case sensitive and the accepted input is upper case.
    ///
    /// # Errors
    /// Returns a `InvalidHttpVersion` when the HTTP version is not supported.
    pub fn try_from(bytes: &[u8]) -> Result<Self, RequestError> {
        match bytes {
            b"HTTP/1.0" => Ok(Self::Http10),
            b"HTTP/1.1" => Ok(Self::Http11),
            _ => Err(RequestError::InvalidHttpVersion(
                "Unsupported HTTP version.",
            )),
        }
    }
}

///Errors associated with a sys errno
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SysError(i32);

impl SysError {
    /// create SysError from errno
    pub fn new(errno: i32) -> SysError {
        SysError(errno)
    }

    /// create SysError from last_os_error
    pub fn last() -> SysError {
        SysError(std::io::Error::last_os_error().raw_os_error().unwrap())
    }

    /// get internal errno
    pub fn errno(self) -> i32 {
        self.0
    }
}

impl Display for SysError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::io::Error::from_raw_os_error(self.0).fmt(f)
    }
}

impl std::error::Error for SysError {}

impl From<std::io::Error> for SysError {
    fn from(e: std::io::Error) -> Self {
        SysError::new(e.raw_os_error().unwrap_or_default())
    }
}

impl From<SysError> for std::io::Error {
    fn from(err: SysError) -> std::io::Error {
        std::io::Error::from_raw_os_error(err.0)
    }
}

pub type SysResult<T> = std::result::Result<T, SysError>;

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq for ConnectionError {
        fn eq(&self, other: &Self) -> bool {
            use self::ConnectionError::*;
            match (self, other) {
                (ParseError(ref e), ParseError(ref other_e)) => e.eq(other_e),
                (ConnectionClosed, ConnectionClosed) => true,
                (StreamReadError(ref e), StreamReadError(ref other_e)) => {
                    format!("{}", e).eq(&format!("{}", other_e))
                }
                (StreamWriteError(ref e), StreamWriteError(ref other_e)) => {
                    format!("{}", e).eq(&format!("{}", other_e))
                }
                (InvalidWrite, InvalidWrite) => true,
                _ => false,
            }
        }
    }

    #[test]
    fn test_version() {
        // Tests for raw()
        assert_eq!(Version::Http10.raw(), b"HTTP/1.0");
        assert_eq!(Version::Http11.raw(), b"HTTP/1.1");

        // Tests for try_from()
        assert_eq!(Version::try_from(b"HTTP/1.0").unwrap(), Version::Http10);
        assert_eq!(Version::try_from(b"HTTP/1.1").unwrap(), Version::Http11);
        assert_eq!(
            Version::try_from(b"HTTP/2.0").unwrap_err(),
            RequestError::InvalidHttpVersion("Unsupported HTTP version.")
        );

        // Test for default()
        assert_eq!(Version::default(), Version::Http11);
    }

    #[test]
    fn test_method() {
        // Test for raw
        assert_eq!(Method::Get.raw(), b"GET");
        assert_eq!(Method::Head.raw(), b"HEAD");
        assert_eq!(Method::Post.raw(), b"POST");
        assert_eq!(Method::Put.raw(), b"PUT");
        assert_eq!(Method::Patch.raw(), b"PATCH");
        assert_eq!(Method::Post.raw(), b"POST");
        assert_eq!(Method::Delete.raw(), b"DELETE");

        // Tests for try_from
        assert_eq!(Method::try_from(b"GET").unwrap(), Method::Get);
        assert_eq!(Method::try_from(b"HEAD").unwrap(), Method::Head);
        assert_eq!(Method::try_from(b"POST").unwrap(), Method::Post);
        assert_eq!(Method::try_from(b"PUT").unwrap(), Method::Put);
        assert_eq!(Method::try_from(b"PATCH").unwrap(), Method::Patch);
        assert_eq!(Method::try_from(b"DELETE").unwrap(), Method::Delete);
        assert_eq!(
            Method::try_from(b"CONNECT").unwrap_err(),
            RequestError::InvalidHttpMethod("Unsupported HTTP method.")
        );
        assert_eq!(Method::try_from(b"POST").unwrap(), Method::Post);
        assert_eq!(Method::try_from(b"DELETE").unwrap(), Method::Delete);
    }

    #[test]
    fn test_body() {
        let body = Body::new("".to_string());
        // Test for is_empty
        assert!(body.is_empty());
        let body = Body::new("This is a body.".to_string());
        // Test for len
        assert_eq!(body.len(), 15);
        // Test for raw
        assert_eq!(body.raw(), b"This is a body.");
    }

    #[test]
    fn test_display_request_error() {
        assert_eq!(
            format!("{}", RequestError::BodyWithoutPendingRequest),
            "No request was pending while the request body was being parsed."
        );
        assert_eq!(
            format!("{}", RequestError::HeadersWithoutPendingRequest),
            "No request was pending while the request headers were being parsed."
        );
        assert_eq!(
            format!("{}", RequestError::InvalidHttpMethod("test")),
            "Invalid HTTP Method: test"
        );
        assert_eq!(
            format!("{}", RequestError::InvalidHttpVersion("test")),
            "Invalid HTTP Version: test"
        );
        assert_eq!(
            format!("{}", RequestError::InvalidRequest),
            "Invalid request."
        );
        assert_eq!(
            format!("{}", RequestError::InvalidUri("test")),
            "Invalid URI: test"
        );
        assert_eq!(
            format!("{}", RequestError::Overflow),
            "Overflow occurred when parsing a request."
        );
        assert_eq!(
            format!("{}", RequestError::Underflow),
            "Underflow occurred when parsing a request."
        );
        assert_eq!(
            format!("{}", RequestError::SizeLimitExceeded(4, 10)),
            "Request payload with size 10 is larger than the limit of 4 allowed by server."
        );
    }

    #[test]
    fn test_display_header_error() {
        assert_eq!(
            format!(
                "{}",
                RequestError::HeaderError(HttpHeaderError::InvalidFormat("test".to_string()))
            ),
            "Invalid header. Reason: Header is incorrectly formatted. Key: test"
        );
        let value = String::from_utf8(vec![0, 159]);
        assert_eq!(
            format!(
                "{}",
                RequestError::HeaderError(HttpHeaderError::InvalidUtf8String(
                    value.unwrap_err().utf8_error()
                ))
            ),
            "Invalid header. Reason: Header contains invalid characters. Key: invalid utf-8 sequence of 1 bytes from index 1"
        );
        assert_eq!(
            format!(
                "{}",
                RequestError::HeaderError(HttpHeaderError::SizeLimitExceeded("test".to_string()))
            ),
            "Invalid header. Reason: Invalid content length. Header: test"
        );
        assert_eq!(
            format!(
                "{}",
                RequestError::HeaderError(HttpHeaderError::UnsupportedFeature(
                    "test".to_string(),
                    "test".to_string()
                ))
            ),
            "Invalid header. Reason: Unsupported feature. Key: test; Value: test"
        );
        assert_eq!(
            format!(
                "{}",
                RequestError::HeaderError(HttpHeaderError::UnsupportedName("test".to_string()))
            ),
            "Invalid header. Reason: Unsupported header name. Key: test"
        );
        assert_eq!(
            format!(
                "{}",
                RequestError::HeaderError(HttpHeaderError::UnsupportedValue(
                    "test".to_string(),
                    "test".to_string()
                ))
            ),
            "Invalid header. Reason: Unsupported value. Key:test; Value:test"
        );
    }

    #[test]
    fn test_display_connection_error() {
        assert_eq!(
            format!("{}", ConnectionError::ConnectionClosed),
            "Connection closed."
        );
        assert_eq!(
            format!(
                "{}",
                ConnectionError::ParseError(RequestError::InvalidRequest)
            ),
            "Parsing error: Invalid request."
        );
        assert_eq!(
            format!("{}", ConnectionError::InvalidWrite),
            "Invalid write attempt."
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            format!(
                "{}",
                ConnectionError::StreamWriteError(std::io::Error::from_raw_os_error(11))
            ),
            "Writing stream error: Resource temporarily unavailable (os error 11)"
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            format!(
                "{}",
                ConnectionError::StreamWriteError(std::io::Error::from_raw_os_error(11))
            ),
            "Writing stream error: Resource deadlock avoided (os error 11)"
        );
    }

    #[test]
    fn test_display_server_error() {
        assert_eq!(
            format!(
                "{}",
                ServerError::ConnectionError(ConnectionError::ConnectionClosed)
            ),
            "Connection error: Connection closed."
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            format!(
                "{}",
                ServerError::IOError(std::io::Error::from_raw_os_error(11))
            ),
            "IO error: Resource temporarily unavailable (os error 11)"
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            format!(
                "{}",
                ServerError::IOError(std::io::Error::from_raw_os_error(11))
            ),
            "IO error: Resource deadlock avoided (os error 11)"
        );
        assert_eq!(
            format!("{}", ServerError::Overflow),
            "Overflow occured while processing messages."
        );
        assert_eq!(format!("{}", ServerError::ServerFull), "Server is full.");
        assert_eq!(
            format!("{}", ServerError::Underflow),
            "Underflow occured while processing messages."
        );
    }

    #[test]
    fn test_display_route_error() {
        assert_eq!(
            format!("{}", RouteError::HandlerExist("test".to_string())),
            "handler for test already exists"
        );
    }

    #[test]
    fn test_method_to_str() {
        let val = Method::Get;
        assert_eq!(val.to_str(), "GET");

        let val = Method::Head;
        assert_eq!(val.to_str(), "HEAD");

        let val = Method::Post;
        assert_eq!(val.to_str(), "POST");

        let val = Method::Put;
        assert_eq!(val.to_str(), "PUT");

        let val = Method::Patch;
        assert_eq!(val.to_str(), "PATCH");

        let val = Method::Delete;
        assert_eq!(val.to_str(), "DELETE");
    }
}
