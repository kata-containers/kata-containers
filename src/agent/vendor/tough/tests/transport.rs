use std::fs;
use std::str::FromStr;
use tempfile::TempDir;
use test_utils::read_to_end;
use tough::{DefaultTransport, Transport, TransportErrorKind};
use url::Url;

mod test_utils;

/// If the `http` feature is not enabled, we should get an error message indicating that the feature
/// is not enabled.
#[cfg(not(feature = "http"))]
#[test]
fn default_transport_error_no_http() {
    let transport = DefaultTransport::new();
    let url = Url::from_str("http://example.com").unwrap();
    let error = transport.fetch(url).err().unwrap();
    match error.kind() {
        TransportErrorKind::UnsupportedUrlScheme => {
            let message = format!("{}", error);
            assert!(message.contains("http feature"))
        }
        _ => panic!("incorrect error kind, expected UnsupportedUrlScheme"),
    }
}

#[test]
fn default_transport_error_ftp() {
    let transport = DefaultTransport::new();
    let url = Url::from_str("ftp://example.com").unwrap();
    let error = transport.fetch(url.clone()).err().unwrap();
    match error.kind() {
        TransportErrorKind::UnsupportedUrlScheme => assert_eq!(error.url(), url.as_str()),
        _ => panic!("incorrect error kind, expected UnsupportedUrlScheme"),
    }
}

#[test]
fn default_transport_file() {
    let dir = TempDir::new().unwrap();
    let filepath = dir.path().join("file.txt");
    fs::write(&filepath, "123123987").unwrap();
    let transport = DefaultTransport::new();
    let url = Url::from_file_path(filepath).unwrap();
    let read = transport.fetch(url).unwrap();
    let temp_vec = read_to_end(read);
    let contents = String::from_utf8_lossy(&temp_vec);
    assert_eq!(contents, "123123987");
}
