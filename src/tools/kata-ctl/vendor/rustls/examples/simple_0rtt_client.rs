use std::sync::Arc;

use std::convert::TryInto;
use std::io::{stdout, Read, Write};
use std::net::TcpStream;

use rustls::{OwnedTrustAnchor, RootCertStore};

fn start_connection(config: &Arc<rustls::ClientConfig>, domain_name: &str) {
    let server_name = domain_name
        .try_into()
        .expect("invalid DNS name");
    let mut conn = rustls::ClientConnection::new(Arc::clone(config), server_name).unwrap();
    let mut sock = TcpStream::connect(format!("{}:443", domain_name)).unwrap();
    sock.set_nodelay(true).unwrap();
    let request = format!(
        "GET / HTTP/1.1\r\n\
         Host: {}\r\n\
         Connection: close\r\n\
         Accept-Encoding: identity\r\n\
         \r\n",
        domain_name
    );

    // If early data is available with this server, then early_data()
    // will yield Some(WriteEarlyData) and WriteEarlyData implements
    // io::Write.  Use this to send the request.
    if let Some(mut early_data) = conn.early_data() {
        early_data
            .write_all(request.as_bytes())
            .unwrap();
    }

    let mut stream = rustls::Stream::new(&mut conn, &mut sock);

    // Complete handshake.
    stream.flush().unwrap();

    // If we didn't send early data, or the server didn't accept it,
    // then send the request as normal.
    if !stream.conn.is_early_data_accepted() {
        stream
            .write_all(request.as_bytes())
            .unwrap();
    }

    let mut plaintext = Vec::new();
    stream
        .read_to_end(&mut plaintext)
        .unwrap();
    stdout().write_all(&plaintext).unwrap();
}

fn main() {
    env_logger::init();

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

    let mut config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    // Enable early data.
    config.enable_early_data = true;
    let config = Arc::new(config);

    // Do two connections. The first will be a normal request, the
    // second will use early data if the server supports it.
    start_connection(&config, "mesalink.io");
    start_connection(&config, "mesalink.io");
}
