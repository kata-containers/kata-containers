// Copyright 2016 Joseph Birr-Pixton.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use core::convert::TryFrom;
extern crate webpki;

static ALL_SIGALGS: &[&webpki::SignatureAlgorithm] = &[
    &webpki::ECDSA_P256_SHA256,
    &webpki::ECDSA_P256_SHA384,
    &webpki::ECDSA_P384_SHA256,
    &webpki::ECDSA_P384_SHA384,
    &webpki::ED25519,
    #[cfg(feature = "alloc")]
    &webpki::RSA_PKCS1_2048_8192_SHA256,
    #[cfg(feature = "alloc")]
    &webpki::RSA_PKCS1_2048_8192_SHA384,
    #[cfg(feature = "alloc")]
    &webpki::RSA_PKCS1_2048_8192_SHA512,
    #[cfg(feature = "alloc")]
    &webpki::RSA_PKCS1_3072_8192_SHA384,
];

/* Checks we can verify netflix's cert chain.  This is notable
 * because they're rooted at a Verisign v1 root. */
#[cfg(feature = "alloc")]
#[test]
pub fn netflix() {
    let ee: &[u8] = include_bytes!("netflix/ee.der");
    let inter = include_bytes!("netflix/inter.der");
    let ca = include_bytes!("netflix/ca.der");

    let anchors = vec![webpki::TrustAnchor::try_from_cert_der(ca).unwrap()];
    let anchors = webpki::TlsServerTrustAnchors(&anchors);

    #[allow(clippy::unreadable_literal)] // TODO: Make this clear.
    let time = webpki::Time::from_seconds_since_unix_epoch(1492441716);

    let cert = webpki::EndEntityCert::try_from(ee).unwrap();
    assert_eq!(
        Ok(()),
        cert.verify_is_valid_tls_server_cert(ALL_SIGALGS, &anchors, &[inter], time)
    );
}

#[test]
pub fn ed25519() {
    let ee: &[u8] = include_bytes!("ed25519/ee.der");
    let ca = include_bytes!("ed25519/ca.der");

    let anchors = vec![webpki::TrustAnchor::try_from_cert_der(ca).unwrap()];
    let anchors = webpki::TlsServerTrustAnchors(&anchors);

    #[allow(clippy::unreadable_literal)] // TODO: Make this clear.
    let time = webpki::Time::from_seconds_since_unix_epoch(1547363522);

    let cert = webpki::EndEntityCert::try_from(ee).unwrap();
    assert_eq!(
        Ok(()),
        cert.verify_is_valid_tls_server_cert(ALL_SIGALGS, &anchors, &[], time)
    );
}

#[test]
fn read_root_with_zero_serial() {
    let ca = include_bytes!("misc/serial_zero.der");
    let _ =
        webpki::TrustAnchor::try_from_cert_der(ca).expect("godaddy cert should parse as anchor");
}

#[test]
fn read_root_with_neg_serial() {
    let ca = include_bytes!("misc/serial_neg.der");
    let _ = webpki::TrustAnchor::try_from_cert_der(ca).expect("idcat cert should parse as anchor");
}

#[cfg(feature = "std")]
#[test]
fn time_constructor() {
    let _ = webpki::Time::try_from(std::time::SystemTime::now()).unwrap();
}
