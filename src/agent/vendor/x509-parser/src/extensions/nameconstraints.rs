use super::GeneralName;
use crate::error::{X509Error, X509Result};
use crate::extensions::parse_generalname;
use asn1_rs::FromDer;
use der_parser::der::*;
use der_parser::error::BerError;
use nom::combinator::{all_consuming, complete, map, opt};
use nom::multi::many1;
use nom::{Err, IResult};

#[derive(Clone, Debug, PartialEq)]
pub struct NameConstraints<'a> {
    pub permitted_subtrees: Option<Vec<GeneralSubtree<'a>>>,
    pub excluded_subtrees: Option<Vec<GeneralSubtree<'a>>>,
}

impl<'a> FromDer<'a, X509Error> for NameConstraints<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_nameconstraints(i).map_err(Err::convert)
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Represents the structure used in the name constraints extensions.
/// The fields minimum and maximum are not supported (openssl also has no support).
pub struct GeneralSubtree<'a> {
    pub base: GeneralName<'a>,
    // minimum: u32,
    // maximum: Option<u32>,
}

pub(crate) fn parse_nameconstraints<'a>(
    i: &'a [u8],
) -> IResult<&'a [u8], NameConstraints, BerError> {
    fn parse_subtree<'a>(i: &'a [u8]) -> IResult<&'a [u8], GeneralSubtree, BerError> {
        parse_der_sequence_defined_g(|input, _| {
            map(parse_generalname, |base| GeneralSubtree { base })(input)
        })(i)
    }
    fn parse_subtrees(i: &[u8]) -> IResult<&[u8], Vec<GeneralSubtree>, BerError> {
        all_consuming(many1(complete(parse_subtree)))(i)
    }

    let (ret, named_constraints) = parse_der_sequence_defined_g(|input, _| {
        let (rem, permitted_subtrees) =
            opt(complete(parse_der_tagged_explicit_g(0, |input, _| {
                parse_subtrees(input)
            })))(input)?;
        let (rem, excluded_subtrees) =
            opt(complete(parse_der_tagged_explicit_g(1, |input, _| {
                parse_subtrees(input)
            })))(rem)?;
        let named_constraints = NameConstraints {
            permitted_subtrees,
            excluded_subtrees,
        };
        Ok((rem, named_constraints))
    })(i)?;

    Ok((ret, named_constraints))
}
