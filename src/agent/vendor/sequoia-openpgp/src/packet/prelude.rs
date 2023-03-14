//! Brings the most relevant types and traits into scope for working
//! with packets.
//!
//! Less often used types and traits that are more likely to lead to a
//! naming conflict are not brought into scope.  For instance, the
//! markers [`PublicParts`], etc. are not imported to avoid potential
//! naming conflicts.  Instead, they should be accessed as
//! [`key::PublicParts`].  And, [`user_attribute::Subpacket`] is not
//! imported, because it is rarely used.  If required, it should be
//! imported explicitly.
//!
//! [`PublicParts`]: key::PublicParts
//! [`user_attribute::Subpacket`]: user_attribute::Subpacket
//!
//! # Examples
//!
//! ```
//! # #![allow(unused_imports)]
//! # use sequoia_openpgp as openpgp;
//! use openpgp::packet::prelude::*;
//! ```

pub use crate::packet::{
    AED,
    Any,
    Body,
    CompressedData,
    Container,
    Header,
    Key,
    Literal,
    MDC,
    Marker,
    OnePassSig,
    PKESK,
    Packet,
    SEIP,
    SKESK,
    Signature,
    Tag,
    Trust,
    Unknown,
    UserAttribute,
    UserID,
    aed::AED1,
    key,
    key::Key4,
    key::SecretKeyMaterial,
    one_pass_sig::OnePassSig3,
    pkesk::PKESK3,
    seip::SEIP1,
    signature,
    signature::Signature4,
    signature::SignatureBuilder,
    skesk::SKESK4,
    skesk::SKESK5,
    user_attribute,
};
