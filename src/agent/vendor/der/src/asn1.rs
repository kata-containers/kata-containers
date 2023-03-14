//! Module containing all of the various ASN.1 built-in types supported by
//! this library.

mod any;
mod bit_string;
mod boolean;
mod choice;
mod context_specific;
mod generalized_time;
mod ia5_string;
mod integer;
mod null;
mod octet_string;
#[cfg(feature = "oid")]
mod oid;
mod optional;
mod printable_string;
mod sequence;
mod sequence_of;
mod set_of;
mod utc_time;
mod utf8_string;

pub use self::{
    any::Any,
    bit_string::{BitString, BitStringIter},
    choice::Choice,
    context_specific::{ContextSpecific, ContextSpecificRef},
    generalized_time::GeneralizedTime,
    ia5_string::Ia5String,
    integer::bigint::UIntBytes,
    null::Null,
    octet_string::OctetString,
    optional::OptionalRef,
    printable_string::PrintableString,
    sequence::Sequence,
    sequence_of::{SequenceOf, SequenceOfIter},
    set_of::{SetOf, SetOfIter},
    utc_time::UtcTime,
    utf8_string::Utf8String,
};

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use self::set_of::SetOfVec;

#[cfg(feature = "oid")]
#[cfg_attr(docsrs, doc(cfg(feature = "oid")))]
pub use const_oid::ObjectIdentifier;
