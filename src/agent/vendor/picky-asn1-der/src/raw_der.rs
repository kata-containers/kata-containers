use serde::{Deserialize, Serialize};

/// User-provided raw DER wrapper.
///
/// Allow user to provide raw DER: no tag is added by serializer and bytes are bumped as it.
/// Note that provided DER header has to be valid to determine length on deserialization.
///
/// # Example
/// ```
/// use picky_asn1_der::Asn1RawDer;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct A {
///     number: u8,
///     user_provided: Asn1RawDer,
/// }
///
/// let plain_a = A {
///     number: 7,
///     user_provided: Asn1RawDer(vec![
///         0x30, 0x08,
///             0x0C, 0x03, 0x41, 0x62, 0x63,
///             0x02, 0x01, 0x05,
///     ]),
/// };
///
/// let serialized_a = picky_asn1_der::to_vec(&plain_a).expect("A to vec");
/// assert_eq!(
///     serialized_a,
///     [
///         0x30, 0x0D,
///             0x02, 0x01, 0x07,
///             0x30, 0x08,
///                 0x0C, 0x03, 0x41, 0x62, 0x63,
///                 0x02, 0x01, 0x05,
///     ]
/// );
///
/// let deserialized_a = picky_asn1_der::from_bytes(&serialized_a).expect("A from bytes");
/// assert_eq!(plain_a, deserialized_a);
///
/// // we can deserialize into a compatible B structure.
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct B {
///     number: u8,
///     tuple: (String, u8),
/// }
///
/// let plain_b = B { number: 7, tuple: ("Abc".to_owned(), 5) };
/// let deserialized_b: B = picky_asn1_der::from_bytes(&serialized_a).expect("B from bytes");
/// assert_eq!(deserialized_b, plain_b);
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, PartialOrd, Hash, Clone)]
pub struct Asn1RawDer(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl Asn1RawDer {
    pub const NAME: &'static str = "Asn1RawDer";
}

#[cfg(test)]
mod tests {
    use super::*;
    use picky_asn1::wrapper::ExplicitContextTag0;

    #[test]
    fn raw_der_behind_application_tag() {
        let encoded = crate::to_vec(&ExplicitContextTag0(Asn1RawDer(vec![0x02, 0x01, 0x07]))).expect("to vec");
        pretty_assertions::assert_eq!(encoded.as_slice(), [0xA0, 0x03, 0x02, 0x01, 0x07]);

        let decoded: ExplicitContextTag0<Asn1RawDer> = crate::from_bytes(&encoded).expect("from bytes");
        pretty_assertions::assert_eq!((decoded.0).0.as_slice(), [0x02, 0x01, 0x07]);
    }
}
