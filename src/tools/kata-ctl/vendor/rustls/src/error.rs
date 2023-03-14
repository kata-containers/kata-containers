use crate::msgs::enums::{AlertDescription, ContentType, HandshakeType};
use crate::rand;

use std::error::Error as StdError;
use std::fmt;
use std::time::SystemTimeError;

/// rustls reports protocol errors using this type.
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// We received a TLS message that isn't valid right now.
    /// `expect_types` lists the message types we can expect right now.
    /// `got_type` is the type we found.  This error is typically
    /// caused by a buggy TLS stack (the peer or this one), a broken
    /// network, or an attack.
    InappropriateMessage {
        /// Which types we expected
        expect_types: Vec<ContentType>,
        /// What type we received
        got_type: ContentType,
    },

    /// We received a TLS handshake message that isn't valid right now.
    /// `expect_types` lists the handshake message types we can expect
    /// right now.  `got_type` is the type we found.
    InappropriateHandshakeMessage {
        /// Which handshake type we expected
        expect_types: Vec<HandshakeType>,
        /// What handshake type we received
        got_type: HandshakeType,
    },

    /// The peer sent us a syntactically incorrect TLS message.
    CorruptMessage,

    /// The peer sent us a TLS message with invalid contents.
    CorruptMessagePayload(ContentType),

    /// The peer didn't give us any certificates.
    NoCertificatesPresented,

    /// The certificate verifier doesn't support the given type of name.
    UnsupportedNameType,

    /// We couldn't decrypt a message.  This is invariably fatal.
    DecryptError,

    /// We couldn't encrypt a message because it was larger than the allowed message size.
    /// This should never happen if the application is using valid record sizes.
    EncryptError,

    /// The peer doesn't support a protocol version/feature we require.
    /// The parameter gives a hint as to what version/feature it is.
    PeerIncompatibleError(String),

    /// The peer deviated from the standard TLS protocol.
    /// The parameter gives a hint where.
    PeerMisbehavedError(String),

    /// We received a fatal alert.  This means the peer is unhappy.
    AlertReceived(AlertDescription),

    /// We received an invalidly encoded certificate from the peer.
    InvalidCertificateEncoding,

    /// We received a certificate with invalid signature type.
    InvalidCertificateSignatureType,

    /// We received a certificate with invalid signature.
    InvalidCertificateSignature,

    /// We received a certificate which includes invalid data.
    InvalidCertificateData(String),

    /// The presented SCT(s) were invalid.
    InvalidSct(sct::Error),

    /// A catch-all error for unlikely errors.
    General(String),

    /// We failed to figure out what time it currently is.
    FailedToGetCurrentTime,

    /// We failed to acquire random bytes from the system.
    FailedToGetRandomBytes,

    /// This function doesn't work until the TLS handshake
    /// is complete.
    HandshakeNotComplete,

    /// The peer sent an oversized record/fragment.
    PeerSentOversizedRecord,

    /// An incoming connection did not support any known application protocol.
    NoApplicationProtocol,

    /// The `max_fragment_size` value supplied in configuration was too small,
    /// or too large.
    BadMaxFragmentSize,
}

fn join<T: fmt::Debug>(items: &[T]) -> String {
    items
        .iter()
        .map(|x| format!("{:?}", x))
        .collect::<Vec<String>>()
        .join(" or ")
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::InappropriateMessage {
                ref expect_types,
                ref got_type,
            } => write!(
                f,
                "received unexpected message: got {:?} when expecting {}",
                got_type,
                join::<ContentType>(expect_types)
            ),
            Self::InappropriateHandshakeMessage {
                ref expect_types,
                ref got_type,
            } => write!(
                f,
                "received unexpected handshake message: got {:?} when expecting {}",
                got_type,
                join::<HandshakeType>(expect_types)
            ),
            Self::CorruptMessagePayload(ref typ) => {
                write!(f, "received corrupt message of type {:?}", typ)
            }
            Self::PeerIncompatibleError(ref why) => write!(f, "peer is incompatible: {}", why),
            Self::PeerMisbehavedError(ref why) => write!(f, "peer misbehaved: {}", why),
            Self::AlertReceived(ref alert) => write!(f, "received fatal alert: {:?}", alert),
            Self::InvalidCertificateEncoding => {
                write!(f, "invalid peer certificate encoding")
            }
            Self::InvalidCertificateSignatureType => {
                write!(f, "invalid peer certificate signature type")
            }
            Self::InvalidCertificateSignature => {
                write!(f, "invalid peer certificate signature")
            }
            Self::InvalidCertificateData(ref reason) => {
                write!(f, "invalid peer certificate contents: {}", reason)
            }
            Self::CorruptMessage => write!(f, "received corrupt message"),
            Self::NoCertificatesPresented => write!(f, "peer sent no certificates"),
            Self::UnsupportedNameType => write!(f, "presented server name type wasn't supported"),
            Self::DecryptError => write!(f, "cannot decrypt peer's message"),
            Self::EncryptError => write!(f, "cannot encrypt message"),
            Self::PeerSentOversizedRecord => write!(f, "peer sent excess record size"),
            Self::HandshakeNotComplete => write!(f, "handshake not complete"),
            Self::NoApplicationProtocol => write!(f, "peer doesn't support any known protocol"),
            Self::InvalidSct(ref err) => write!(f, "invalid certificate timestamp: {:?}", err),
            Self::FailedToGetCurrentTime => write!(f, "failed to get current time"),
            Self::FailedToGetRandomBytes => write!(f, "failed to get random bytes"),
            Self::BadMaxFragmentSize => {
                write!(f, "the supplied max_fragment_size was too small or large")
            }
            Self::General(ref err) => write!(f, "unexpected error: {}", err),
        }
    }
}

impl From<SystemTimeError> for Error {
    #[inline]
    fn from(_: SystemTimeError) -> Self {
        Self::FailedToGetCurrentTime
    }
}

impl StdError for Error {}

impl From<rand::GetRandomFailed> for Error {
    fn from(_: rand::GetRandomFailed) -> Self {
        Self::FailedToGetRandomBytes
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn smoke() {
        use crate::msgs::enums::{AlertDescription, ContentType, HandshakeType};
        use sct;

        let all = vec![
            Error::InappropriateMessage {
                expect_types: vec![ContentType::Alert],
                got_type: ContentType::Handshake,
            },
            Error::InappropriateHandshakeMessage {
                expect_types: vec![HandshakeType::ClientHello, HandshakeType::Finished],
                got_type: HandshakeType::ServerHello,
            },
            Error::CorruptMessage,
            Error::CorruptMessagePayload(ContentType::Alert),
            Error::NoCertificatesPresented,
            Error::DecryptError,
            Error::PeerIncompatibleError("no tls1.2".to_string()),
            Error::PeerMisbehavedError("inconsistent something".to_string()),
            Error::AlertReceived(AlertDescription::ExportRestriction),
            Error::InvalidCertificateEncoding,
            Error::InvalidCertificateSignatureType,
            Error::InvalidCertificateSignature,
            Error::InvalidCertificateData("Data".into()),
            Error::InvalidSct(sct::Error::MalformedSct),
            Error::General("undocumented error".to_string()),
            Error::FailedToGetCurrentTime,
            Error::FailedToGetRandomBytes,
            Error::HandshakeNotComplete,
            Error::PeerSentOversizedRecord,
            Error::NoApplicationProtocol,
            Error::BadMaxFragmentSize,
        ];

        for err in all {
            println!("{:?}:", err);
            println!("  fmt '{}'", err);
        }
    }

    #[test]
    fn rand_error_mapping() {
        use super::rand;
        let err: Error = rand::GetRandomFailed.into();
        assert_eq!(err, Error::FailedToGetRandomBytes);
    }

    #[test]
    fn time_error_mapping() {
        use std::time::SystemTime;

        let time_error = SystemTime::UNIX_EPOCH
            .duration_since(SystemTime::now())
            .unwrap_err();
        let err: Error = time_error.into();
        assert_eq!(err, Error::FailedToGetCurrentTime);
    }
}
