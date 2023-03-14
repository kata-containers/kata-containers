//! Tests for custom derive support
// TODO(tarcieri): test all types supported by `der_derive`

#![cfg(feature = "derive")]

use der::{Choice, Decodable, Encodable, Encoder, GeneralizedTime, UtcTime};
use hex_literal::hex;
use std::time::Duration;

/// Custom derive test case for the `Choice` mcaro.
///
/// Based on `Time` as defined in RFC 5280:
/// <https://tools.ietf.org/html/rfc5280#page-117>
///
/// ```text
/// Time ::= CHOICE {
///      utcTime        UTCTime,
///      generalTime    GeneralizedTime }
/// ```
#[derive(Choice)]
pub enum Time {
    #[asn1(type = "UTCTime")]
    UtcTime(UtcTime),

    #[asn1(type = "GeneralizedTime")]
    GeneralTime(GeneralizedTime),
}

impl Time {
    fn unix_duration(self) -> Duration {
        match self {
            Time::UtcTime(t) => t.unix_duration(),
            Time::GeneralTime(t) => t.unix_duration(),
        }
    }
}

const UTC_TIMESTAMP: &[u8] = &hex!("17 0d 39 31 30 35 30 36 32 33 34 35 34 30 5a");
const GENERAL_TIMESTAMP: &[u8] = &hex!("18 0f 31 39 39 31 30 35 30 36 32 33 34 35 34 30 5a");

#[test]
fn decode_enum_variants() {
    let utc_time = Time::from_der(UTC_TIMESTAMP).unwrap();
    assert_eq!(utc_time.unix_duration().as_secs(), 673573540);

    let general_time = Time::from_der(GENERAL_TIMESTAMP).unwrap();
    assert_eq!(general_time.unix_duration().as_secs(), 673573540);
}

#[test]
fn encode_enum_variants() {
    let mut buf = [0u8; 128];

    let utc_time = Time::from_der(UTC_TIMESTAMP).unwrap();
    let mut encoder = Encoder::new(&mut buf);
    utc_time.encode(&mut encoder).unwrap();
    assert_eq!(UTC_TIMESTAMP, encoder.finish().unwrap());

    let general_time = Time::from_der(GENERAL_TIMESTAMP).unwrap();
    let mut encoder = Encoder::new(&mut buf);
    general_time.encode(&mut encoder).unwrap();
    assert_eq!(GENERAL_TIMESTAMP, encoder.finish().unwrap());
}
