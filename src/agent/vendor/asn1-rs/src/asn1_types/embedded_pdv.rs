use crate::*;
use core::convert::TryFrom;

#[derive(Debug, PartialEq)]
pub struct EmbeddedPdv<'a> {
    pub identification: PdvIdentification<'a>,
    pub data_value_descriptor: Option<ObjectDescriptor<'a>>,
    pub data_value: &'a [u8],
}

#[derive(Debug, PartialEq)]
pub enum PdvIdentification<'a> {
    Syntaxes {
        s_abstract: Oid<'a>,
        s_transfer: Oid<'a>,
    },
    Syntax(Oid<'a>),
    PresentationContextId(Integer<'a>),
    ContextNegotiation {
        presentation_context_id: Integer<'a>,
        presentation_syntax: Oid<'a>,
    },
    TransferSyntax(Oid<'a>),
    Fixed,
}

impl<'a> TryFrom<Any<'a>> for EmbeddedPdv<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for EmbeddedPdv<'a> {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<Self> {
        let data = any.data;
        // AUTOMATIC TAGS means all values will be tagged (IMPLICIT)
        // [0] -> identification
        let (rem, seq0) =
            TaggedParser::<Explicit, Any>::parse_ber(Class::ContextSpecific, Tag(0), data)?;
        let inner = seq0.inner;
        let identification = match inner.tag() {
            Tag(0) => {
                // syntaxes SEQUENCE {
                //     abstract OBJECT IDENTIFIER,
                //     transfer OBJECT IDENTIFIER
                // },
                // AUTOMATIC tags -> implicit! Hopefully, Oid does not check tag value!
                let (rem, s_abstract) = Oid::from_ber(inner.data)?;
                let (_, s_transfer) = Oid::from_ber(rem)?;
                PdvIdentification::Syntaxes {
                    s_abstract,
                    s_transfer,
                }
            }
            Tag(1) => {
                // syntax OBJECT IDENTIFIER
                let oid = Oid::new(inner.data.into());
                PdvIdentification::Syntax(oid)
            }
            Tag(2) => {
                // presentation-context-id INTEGER
                let i = Integer::new(inner.data);
                PdvIdentification::PresentationContextId(i)
            }
            Tag(3) => {
                // context-negotiation SEQUENCE {
                //     presentation-context-id INTEGER,
                //     transfer-syntax OBJECT IDENTIFIER
                // },
                // AUTOMATIC tags -> implicit!
                let (rem, any) = Any::from_ber(inner.data)?;
                let presentation_context_id = Integer::new(any.data);
                let (_, presentation_syntax) = Oid::from_ber(rem)?;
                PdvIdentification::ContextNegotiation {
                    presentation_context_id,
                    presentation_syntax,
                }
            }
            Tag(4) => {
                // transfer-syntax OBJECT IDENTIFIER
                let oid = Oid::new(inner.data.into());
                PdvIdentification::TransferSyntax(oid)
            }
            Tag(5) => {
                // fixed NULL
                PdvIdentification::Fixed
            }
            _ => {
                return Err(inner
                    .tag()
                    .invalid_value("Invalid identification tag in EMBEDDED PDV"))
            }
        };
        // [1] -> data-value-descriptor ObjectDescriptor OPTIONAL
        // *BUT* WITH COMPONENTS data-value-descriptor ABSENT
        // XXX this should be parse_ber?
        // let (rem, data_value_descriptor) =
        //     TaggedOptional::from(1).parse_der(rem, |_, inner| ObjectDescriptor::from_ber(inner))?;
        let (rem, data_value_descriptor) = (rem, None);
        // [2] -> data-value OCTET STRING
        let (_, data_value) =
            TaggedParser::<Implicit, &[u8]>::parse_ber(Class::ContextSpecific, Tag(2), rem)?;
        let data_value = data_value.inner;
        let obj = EmbeddedPdv {
            identification,
            data_value_descriptor,
            data_value,
        };
        Ok(obj)
    }
}

impl CheckDerConstraints for EmbeddedPdv<'_> {
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.length().assert_definite()?;
        any.header.assert_constructed()?;
        Ok(())
    }
}

impl DerAutoDerive for EmbeddedPdv<'_> {}
