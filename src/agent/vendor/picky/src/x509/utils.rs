use serde::{Deserialize, Serialize};

use super::certificate::CertError;
use crate::pem::{parse_pem, Pem};

pub(super) fn from_der<'a, T, V>(data: &'a V, element: &'static str) -> Result<T, CertError>
where
    T: Deserialize<'a>,
    V: ?Sized + AsRef<[u8]>,
{
    picky_asn1_der::from_bytes(data.as_ref()).map_err(|e| CertError::Asn1Deserialization { source: e, element })
}

pub(super) fn from_pem<'a, T: Deserialize<'a>>(
    pem: &'a Pem,
    valid_pem_labels: &[&str],
    element: &'static str,
) -> Result<T, CertError> {
    if valid_pem_labels.iter().any(|&label| pem.label() == label) {
        from_der(pem.data(), element)
    } else {
        Err(CertError::InvalidPemLabel {
            label: pem.label().to_owned(),
        })
    }
}

pub(super) fn from_pem_str<T>(pem_str: &str, valid_pem_labels: &[&str], element: &'static str) -> Result<T, CertError>
where
    for<'a> T: Deserialize<'a>,
{
    let pem = parse_pem(pem_str)?;
    from_pem(&pem, valid_pem_labels, element)
}

pub(super) fn to_der<T: Serialize>(val: &T, element: &'static str) -> Result<Vec<u8>, CertError> {
    picky_asn1_der::to_vec(val).map_err(|e| CertError::Asn1Serialization { source: e, element })
}

pub(super) fn to_pem<T: Serialize>(val: &T, pem_label: &str, element: &'static str) -> Result<Pem<'static>, CertError> {
    Ok(Pem::new(pem_label, to_der(val, element)?))
}
