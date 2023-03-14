/// This is the simplest possible client using rustls that does something useful:
/// it accepts the default configuration, loads some root certs, and then connects
/// to google.com and issues a basic HTTP request.  The response is printed to stdout.
///
/// It makes use of rustls::Stream to treat the underlying TLS connection as a basic
/// bi-directional stream -- the underlying IO is performed transparently.
///
/// Note that `unwrap()` is used to deal with networking errors; this is not something
/// that is sensible outside of example code.
use std::sync::Arc;

use std::convert::TryInto;
use std::io::{stdout, Read, Write};
use std::net::TcpStream;

use rustls::{OwnedTrustAnchor, RootCertStore};

fn main() {
    let mut root_store = RootCertStore::empty();
    root_store.add_server_trust_anchors(
        webpki_roots::TLS_SERVER_ROOTS
            .0
            .iter()
            .map(|ta| {
                OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }),
    );
    let config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let server_name = "google.com".try_into().unwrap();
    let mut conn = rustls::ClientConnection::new(Arc::new(config), server_name).unwrap();
    let mut sock = TcpStream::connect("google.com:443").unwrap();
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);
    tls.write_all(
        concat!(
            "GET / HTTP/1.1\r\n",
            "Host: google.com\r\n",
            "Connection: close\r\n",
            "Accept-Encoding: identity\r\n",
            "\r\n"
        )
        .as_bytes(),
    )
    .unwrap();
    let ciphersuite = tls
        .conn
        .negotiated_cipher_suite()
        .unwrap();
    writeln!(
        &mut std::io::stderr(),
        "Current ciphersuite: {:?}",
        ciphersuite.suite()
    )
    .unwrap();
    let mut plaintext = Vec::new();
    tls.read_to_end(&mut plaintext).unwrap();
    stdout().write_all(&plaintext).unwrap();
}
