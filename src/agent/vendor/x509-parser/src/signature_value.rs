use asn1_rs::{DerSequence, Integer};

/// ECDSA Signature Value (RFC3279)
// Ecdsa-Sig-Value  ::=  SEQUENCE  {
//     r     INTEGER,
//     s     INTEGER  }
#[derive(Debug, PartialEq, DerSequence)]
pub struct EcdsaSigValue<'a> {
    pub r: Integer<'a>,
    pub s: Integer<'a>,
}
