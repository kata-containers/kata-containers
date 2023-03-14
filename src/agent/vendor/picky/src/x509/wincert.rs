use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::error;
use std::io::{self, BufReader, BufWriter, Write};
use thiserror::Error;

const MINIMUM_BYTES_TO_DECODE: usize = 4 /* WinCertificate::length */ + 2 /* WinCertificate::revision */ + 2 /* WinCertificate::certificate */;

#[derive(Debug, Error)]
pub enum WinCertificateError {
    #[error("Revision value is wrong(expected any of {expected}, but {got} got)")]
    WrongRevisionValue { expected: String, got: u16 },
    #[error("Certificate type is wrong(expected any of {expected}, but {got} got)")]
    WrongCertificateType { expected: String, got: u16 },
    #[error("Length is wrong({minimum} at least, but {got} got)")]
    WrongLength { minimum: usize, got: usize },
    #[error("Certificate data is empty")]
    CertificateDataIsEmpty,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Other(#[from] Box<dyn error::Error>),
}

pub type WinCertificateResult<T> = Result<T, WinCertificateError>;

#[derive(Clone, Debug, PartialEq)]
pub struct WinCertificate {
    length: u32,
    revision: RevisionType,
    certificate_type: CertificateType,
    certificate: Vec<u8>,
}

impl WinCertificate {
    pub fn decode<V: ?Sized + AsRef<[u8]>>(data: &V) -> WinCertificateResult<Self> {
        if data.as_ref().len() < MINIMUM_BYTES_TO_DECODE {
            return Err(WinCertificateError::WrongLength {
                minimum: MINIMUM_BYTES_TO_DECODE,
                got: data.as_ref().len(),
            });
        }

        let mut buffer = BufReader::with_capacity(data.as_ref().len(), data.as_ref());

        let length = buffer.read_u32::<LittleEndian>()?;

        if length == 0 {
            return Err(WinCertificateError::CertificateDataIsEmpty);
        }

        let revision = RevisionType::try_from(buffer.read_u16::<LittleEndian>()?)?;

        let certificate_type = CertificateType::try_from(buffer.read_u16::<LittleEndian>()?)?;

        let certificate_length = length as usize - MINIMUM_BYTES_TO_DECODE;
        let mut certificate = Vec::with_capacity(certificate_length);

        for _ in 0..certificate_length {
            certificate.push(buffer.read_u8()?);
        }

        Ok(Self {
            length,
            revision,
            certificate_type,
            certificate,
        })
    }

    pub fn encode(self) -> WinCertificateResult<Vec<u8>> {
        let Self {
            length,
            revision,
            certificate_type,
            certificate,
        } = self;

        let padding = (8 - (certificate.len() % 8)) % 8;

        let mut buffer = BufWriter::with_capacity(length as usize + padding, Vec::new());

        buffer.write_u32::<LittleEndian>(length + padding as u32)?;
        buffer.write_u16::<LittleEndian>(revision as u16)?;
        buffer.write_u16::<LittleEndian>(certificate_type as u16)?;

        buffer.write_all(&certificate)?;
        buffer.write_all(&vec![0; padding as usize])?;

        buffer
            .into_inner()
            .map_err(|err| WinCertificateError::Other(Box::new(err) as Box<dyn error::Error>))
    }

    pub fn from_certificate<V: Into<Vec<u8>>>(certificate: V, certificate_type: CertificateType) -> Self {
        let certificate = certificate.into();
        Self {
            // According to Authenticode_PE.docx WinCertificate::length is set to the WinCertificate::certificate length,
            // but if the length does not include other WinCertificate's fields (WinCertificate::revision,
            // WinCertificate::certificate_type and WinCertificate::length itself) sizes the signature will invalid.
            // MSDN says the same:
            // dwLength - Specifies the length, in bytes, of the signature
            // (see https://docs.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-win_certificate).
            length: (MINIMUM_BYTES_TO_DECODE + certificate.len()) as u32,
            revision: RevisionType::WinCertificateRevision20,
            certificate_type,
            certificate,
        }
    }

    #[inline]
    pub fn get_certificate(&self) -> &[u8] {
        self.certificate.as_ref()
    }
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u16)]
pub enum RevisionType {
    WinCertificateRevision10 = 0x0100,
    WinCertificateRevision20 = 0x0200,
}

impl TryFrom<u16> for RevisionType {
    type Error = WinCertificateError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0100 => Ok(RevisionType::WinCertificateRevision10),
            0x0200 => Ok(RevisionType::WinCertificateRevision20),
            _ => Err(WinCertificateError::WrongRevisionValue {
                expected: format!("{:?}", [0x0100, 0x0200]),
                got: value,
            }),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u16)]
pub enum CertificateType {
    WinCertTypeX509 = 0x0001,
    WinCertTypePkcsSignedData = 0x0002,
    WinCertTypeReserved1 = 0x0003,
    WinCertTypePkcs1Sign = 0x0009,
}

impl TryFrom<u16> for CertificateType {
    type Error = WinCertificateError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(CertificateType::WinCertTypeX509),
            0x0002 => Ok(CertificateType::WinCertTypePkcsSignedData),
            0x0003 => Ok(CertificateType::WinCertTypeReserved1),
            0x0009 => Ok(CertificateType::WinCertTypePkcs1Sign),
            _ => Err(WinCertificateError::WrongCertificateType {
                expected: format!("{:?}", [0x0001, 0x0002, 0x0003, 0x0009]),
                got: value,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINCERT_WITH_INVALID_LENGTH: [u8; 8] = [
        0x00, 0x00, 0x00, 0x00, // -> WIN_CERTIFICATE::dwLength = 0x00 = 0
        0x00, 0x01, // -> WIN_CERTIFICATE::wRevision = 0x0100 = WIN_CERT_REVISION_1_0
        0x01,
        0x00, // -> WIN_CERTIFICATE::wCertificateType = 0x01 = WIN_CERTIFICATE::WIN_CERT_TYPE_X509
              // empty WIN_CERTIFICATE::bCertificate field
    ];

    const WINCERT_WITH_ONE_BYTE_CERTIFICATE: [u8; 16] = [
        0x10, 0x00, 0x00, 0x00, // -> WIN_CERTIFICATE::dwLength = 0x10 = 16
        0x00, 0x01, // -> WIN_CERTIFICATE::wRevision = 0x0100 = WIN_CERT_REVISION_1_0
        0x01, 0x00, // -> WIN_CERTIFICATE::wCertificateType = 0x01 = WIN_CERTIFICATE::WIN_CERT_TYPE_X509
        0x01, // -> WIN_CERTIFICATE::bCertificate = bCertificate[0] = 1
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // padding
    ];

    const WINCERT_WITH_TEN_BYTES_CERTIFICATE: [u8; 24] = [
        0x18, 0x00, 0x00, 0x00, // -> WIN_CERTIFICATE::dwLength = 0x18 = 24
        0x00, 0x02, // -> WIN_CERTIFICATE::wRevision = 0x0200 = WIN_CERT_REVISION_2_0
        0x09, 0x00, // -> WIN_CERTIFICATE::wCertificateType = 0x09 = WIN_CERTIFICATE::WinCertTypePkcs1Sign
        0x01, 0x20, 0x03, 0x40, 0x05, 0x60, 0x70, 0x08, // -> WIN_CERTIFICATE::bCertificate = bCertificate[10]
        0x90, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // padding
    ];

    const WINCERT_WITH_INVALID_REVISION: [u8; 9] = [
        0x09, 0x00, 0x00, 0x00, // -> WIN_CERTIFICATE::dwLength = 0x09 = 9
        0x00, 0x03, // -> WIN_CERTIFICATE::wRevision = 0x0300(not existing)
        0x01, 0x00, // -> WIN_CERTIFICATE::wCertificateType = 0x01 = WIN_CERTIFICATE::WIN_CERT_TYPE_X509
        0x01, // -> WIN_CERTIFICATE::bCertificate = bCertificate[0] = 1
    ];

    const WINCERT_WITH_X509_CERTIFICATE: [u8; 136] = [
        0x88, 0x00, 0x00, 0x00, // -> WIN_CERTIFICATE::dwLength = 0x80 = 136
        0x00, 0x02, // -> WIN_CERTIFICATE::wRevision = 0x0200 = WIN_CERT_REVISION_2_0
        0x01, 0x00, // -> WIN_CERTIFICATE::wCertificateType = 0x01 = WIN_CERTIFICATE::WIN_CERT_TYPE_X509
        // X509 certificate
        0x0B, 0x04, 0x55, 0x03, 0x06, 0x15, 0x30, 0x17, 0x31, 0x74, 0x69, 0x72, 0x6F, 0x69, 0x72, 0x70, 0x41, 0x08,
        0x0C, 0x0A, 0x04, 0x55, 0x03, 0x06, 0x0F, 0x30, 0x11, 0x31, 0x6F, 0x72, 0x70, 0x69, 0x6E, 0x44, 0x06, 0x0C,
        0x07, 0x04, 0x55, 0x03, 0x06, 0x0D, 0x30, 0x0F, 0x31, 0x6F, 0x72, 0x70, 0x69, 0x6E, 0x44, 0x06, 0x0C, 0x08,
        0x04, 0x55, 0x03, 0x06, 0x0D, 0x30, 0x0F, 0x31, 0x41, 0x55, 0x02, 0x13, 0x06, 0x04, 0x55, 0x03, 0x06, 0x09,
        0x30, 0x0B, 0x31, 0x97, 0x81, 0x30, 0x00, 0x05, 0x0B, 0x01, 0x01, 0x0D, 0xF7, 0x86, 0x48, 0x86, 0x2A, 0x09,
        0x06, 0x0D, 0x30, 0x92, 0x72, 0xDC, 0xE9, 0x6B, 0x13, 0x8F, 0x0D, 0x06, 0xFB, 0xC1, 0xD1, 0x69, 0x97, 0x79,
        0x4B, 0xA1, 0x69, 0x63, 0x19, 0x14, 0x02, 0x02, 0x01, 0x02, 0x03, 0xA0, 0xF9, 0x03, 0x82, 0x30, 0x11, 0x06,
        0x82, 0x30,
    ];

    #[test]
    fn decode_with_invalid_length() {
        let decoded = WinCertificate::decode(WINCERT_WITH_INVALID_LENGTH.as_ref());
        assert!(decoded.is_err());
    }

    #[test]
    fn decode_wincert_with_one_byte_certificate() {
        let decoded = WinCertificate::decode(WINCERT_WITH_ONE_BYTE_CERTIFICATE.as_ref()).unwrap();

        pretty_assertions::assert_eq!(decoded.length, 16);
        pretty_assertions::assert_eq!(decoded.revision, RevisionType::WinCertificateRevision10);
        pretty_assertions::assert_eq!(decoded.certificate_type, CertificateType::WinCertTypeX509);
        pretty_assertions::assert_eq!(decoded.certificate, vec![1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn encode_into_wincert_with_one_byte_certificate() {
        let wincert = WinCertificate {
            length: 9,
            revision: RevisionType::WinCertificateRevision10,
            certificate_type: CertificateType::WinCertTypeX509,
            certificate: vec![1],
        };

        let encoded = wincert.encode().unwrap();
        assert_eq!(encoded, WINCERT_WITH_ONE_BYTE_CERTIFICATE.to_vec());
    }

    #[test]
    fn decode_wincert_with_ten_bytes_certificate() {
        let decoded = WinCertificate::decode(WINCERT_WITH_TEN_BYTES_CERTIFICATE.as_ref()).unwrap();

        pretty_assertions::assert_eq!(decoded.length, 24);
        pretty_assertions::assert_eq!(decoded.revision, RevisionType::WinCertificateRevision20);
        pretty_assertions::assert_eq!(decoded.certificate_type, CertificateType::WinCertTypePkcs1Sign);
        pretty_assertions::assert_eq!(
            decoded.certificate,
            vec![1, 32, 3, 64, 5, 96, 112, 8, 144, 1, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn encode_into_wincert_with_ten_bytes_certificate() {
        let wincert = WinCertificate {
            length: 18,
            revision: RevisionType::WinCertificateRevision20,
            certificate_type: CertificateType::WinCertTypePkcs1Sign,
            certificate: vec![1, 32, 3, 64, 5, 96, 112, 8, 144, 1],
        };

        let encoded = wincert.encode().unwrap();
        assert_eq!(encoded, WINCERT_WITH_TEN_BYTES_CERTIFICATE.to_vec());
    }

    #[test]
    fn decode_wincert_with_invalid_revision() {
        let decoded = WinCertificate::decode(WINCERT_WITH_INVALID_REVISION.as_ref());
        assert!(decoded.is_err());
    }

    #[test]
    fn decode_wincert_with_x509_certificate() {
        WinCertificate::decode(WINCERT_WITH_X509_CERTIFICATE.as_ref()).unwrap();
    }

    #[test]
    fn encode_wincert_with_x509_certificate() {
        let wincert = WinCertificate {
            length: 136,
            revision: RevisionType::WinCertificateRevision20,
            certificate_type: CertificateType::WinCertTypeX509,
            certificate: WINCERT_WITH_X509_CERTIFICATE[8..].to_vec(),
        };

        let encoded = wincert.encode().unwrap();
        pretty_assertions::assert_eq!(encoded, WINCERT_WITH_X509_CERTIFICATE.to_vec());
    }
}
