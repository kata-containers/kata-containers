use oid_registry::{OID_PKCS1_SHA256WITHRSA, OID_SIG_ECDSA_WITH_SHA256, OID_X509_COMMON_NAME};
use x509_parser::prelude::*;

const CSR_DATA_EMPTY_ATTRIB: &[u8] = include_bytes!("../assets/csr-empty-attributes.csr");
const CSR_DATA: &[u8] = include_bytes!("../assets/test.csr");

#[test]
fn read_csr_empty_attrib() {
    let (rem, csr) =
        X509CertificationRequest::from_der(CSR_DATA_EMPTY_ATTRIB).expect("could not parse CSR");

    assert!(rem.is_empty());
    let cri = &csr.certification_request_info;
    assert_eq!(cri.version, X509Version(0));
    assert_eq!(cri.attributes().len(), 0);
    assert_eq!(csr.signature_algorithm.algorithm, OID_PKCS1_SHA256WITHRSA);
}

#[test]
fn read_csr_with_san() {
    let der = pem::parse_x509_pem(CSR_DATA).unwrap().1;
    let (rem, csr) =
        X509CertificationRequest::from_der(&der.contents).expect("could not parse CSR");

    assert!(rem.is_empty());
    let cri = &csr.certification_request_info;
    assert_eq!(cri.version, X509Version(0));
    assert_eq!(cri.attributes().len(), 1);
    assert_eq!(csr.signature_algorithm.algorithm, OID_SIG_ECDSA_WITH_SHA256);

    let mut rdns = cri.subject.iter();
    let rdn = rdns.next().unwrap();
    let first = rdn.iter().next().unwrap();
    assert_eq!(first.attr_type(), &OID_X509_COMMON_NAME);
    assert_eq!(first.as_str().unwrap(), "test.rusticata.fr");

    let expected: &[u8] = &[
        4, 195, 245, 126, 177, 113, 192, 146, 215, 136, 181, 58, 82, 138, 142, 61, 253, 245, 185,
        192, 166, 216, 218, 145, 219, 42, 169, 112, 122, 58, 91, 184, 150, 37, 237, 245, 59, 54,
        44, 210, 44, 207, 218, 167, 148, 189, 210, 159, 207, 103, 233, 1, 187, 134, 137, 24, 240,
        188, 223, 135, 215, 71, 80, 64, 65,
    ];
    assert_eq!(cri.subject_pki.subject_public_key.data, expected);

    let mut extensions = csr.requested_extensions().unwrap();
    match extensions.next().unwrap() {
        ParsedExtension::SubjectAlternativeName(san) => {
            let name = san.general_names.first().unwrap();
            assert!(matches!(name, GeneralName::DNSName("test.rusticata.fr")));
        }
        _ => unreachable!(),
    }
}

#[cfg(feature = "verify")]
#[test]
fn read_csr_verify() {
    let der = pem::parse_x509_pem(CSR_DATA).unwrap().1;
    let (_, csr) = X509CertificationRequest::from_der(&der.contents).expect("could not parse CSR");
    csr.verify_signature().unwrap();

    let mut der = pem::parse_x509_pem(CSR_DATA).unwrap().1;
    assert_eq!(&der.contents[28..37], b"rusticata");
    for (i, b) in b"foobarbaz".iter().enumerate() {
        der.contents[28 + i] = *b;
    }
    assert_eq!(&der.contents[28..37], b"foobarbaz");

    let (_, csr) = X509CertificationRequest::from_der(&der.contents).expect("could not parse CSR");
    csr.verify_signature().unwrap_err();
}
