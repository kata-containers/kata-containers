use std::io::Cursor;
use x509_parser::pem::{parse_x509_pem, Pem};
use x509_parser::{parse_x509_certificate, x509::X509Version};

static IGCA_PEM: &[u8] = include_bytes!("../assets/IGC_A.pem");

#[test]
fn test_x509_parse_pem() {
    let (rem, pem) = parse_x509_pem(IGCA_PEM).expect("PEM parsing failed");
    // println!("{:?}", pem);
    assert!(rem.is_empty());
    assert_eq!(pem.label, String::from("CERTIFICATE"));
    //
    // now check that the content is indeed a certificate
    let (rem, crt) = parse_x509_certificate(&pem.contents).expect("X.509 parsing failed");
    // println!("res: {:?}", res);
    assert!(rem.is_empty());
    assert_eq!(crt.tbs_certificate.version, X509Version::V3);
}

#[test]
fn test_pem_read() {
    let reader = Cursor::new(IGCA_PEM);
    let (pem, bytes_read) = Pem::read(reader).expect("Reading PEM failed");
    // println!("{:?}", pem);
    assert_eq!(bytes_read, IGCA_PEM.len());
    assert_eq!(pem.label, String::from("CERTIFICATE"));
    //
    // now check that the content is indeed a certificate
    let x509 = pem.parse_x509().expect("X.509: decoding DER failed");
    assert_eq!(x509.tbs_certificate.version, X509Version::V3);
}

#[test]
fn test_pem_not_pem() {
    let bytes = vec![0x1, 0x2, 0x3, 0x4, 0x5];
    let reader = Cursor::new(bytes);
    let res = Pem::read(reader);
    assert!(res.is_err());
}

static NO_END: &[u8] = include_bytes!("../assets/no_end.pem");

#[test]
fn test_pem_no_end() {
    let reader = Cursor::new(NO_END);
    let res = Pem::read(reader);
    assert!(res.is_err());
}
