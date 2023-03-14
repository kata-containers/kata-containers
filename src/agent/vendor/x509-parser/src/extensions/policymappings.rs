use crate::error::{X509Error, X509Result};
use asn1_rs::{DerSequence, Error, FromDer, Oid};
use nom::{Err, IResult};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyMappings<'a> {
    pub mappings: Vec<PolicyMapping<'a>>,
}

impl<'a> FromDer<'a, X509Error> for PolicyMappings<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_policymappings(i).map_err(Err::convert)
    }
}

impl<'a> PolicyMappings<'a> {
    /// Returns a `HashMap` mapping `Oid` to the list of references to `Oid`
    ///
    /// If several names match the same `Oid`, they are merged in the same entry.
    pub fn as_hashmap(&self) -> HashMap<Oid<'a>, Vec<&Oid<'a>>> {
        // create the hashmap and merge entries with same OID
        let mut m: HashMap<Oid, Vec<&_>> = HashMap::new();
        for desc in &self.mappings {
            let PolicyMapping {
                issuer_domain_policy: left,
                subject_domain_policy: right,
            } = desc;
            if let Some(l) = m.get_mut(left) {
                l.push(right);
            } else {
                m.insert(left.clone(), vec![right]);
            }
        }
        m
    }

    /// Returns a `HashMap` mapping `Oid` to the list of `Oid` (consuming the input)
    ///
    /// If several names match the same `Oid`, they are merged in the same entry.
    pub fn into_hashmap(self) -> HashMap<Oid<'a>, Vec<Oid<'a>>> {
        let mut l = self.mappings;
        // create the hashmap and merge entries with same OID
        let mut m: HashMap<Oid, Vec<_>> = HashMap::new();
        for mapping in l.drain(..) {
            let PolicyMapping {
                issuer_domain_policy: left,
                subject_domain_policy: right,
            } = mapping;
            if let Some(general_names) = m.get_mut(&left) {
                general_names.push(right);
            } else {
                m.insert(left, vec![right]);
            }
        }
        m
    }
}

#[derive(Clone, Debug, PartialEq, DerSequence)]
pub struct PolicyMapping<'a> {
    pub issuer_domain_policy: Oid<'a>,
    pub subject_domain_policy: Oid<'a>,
}

impl<'a> PolicyMapping<'a> {
    pub const fn new(issuer_domain_policy: Oid<'a>, subject_domain_policy: Oid<'a>) -> Self {
        PolicyMapping {
            issuer_domain_policy,
            subject_domain_policy,
        }
    }
}

// PolicyMappings ::= SEQUENCE SIZE (1..MAX) OF SEQUENCE {
//  issuerDomainPolicy      CertPolicyId,
//  subjectDomainPolicy     CertPolicyId }
pub(crate) fn parse_policymappings(i: &[u8]) -> IResult<&[u8], PolicyMappings, Error> {
    let (ret, pairs) = <Vec<PolicyMapping>>::from_der(i)?;
    // let mut mappings: HashMap<Oid, Vec<Oid>> = HashMap::new();
    let mappings = pairs;
    // let mut mappings = Vec::new();
    // for pair in pairs.iter() {
    //     // XXX this should go to Validate
    //     // if left.bytes() == oid!(raw 2.5.29.32.0) || right.bytes() == oid!(raw 2.5.29.32.0) {
    //     //     // mapping to or from anyPolicy is not allowed
    //     //     return Err(Err::Failure(BerError::InvalidTag));
    //     // }
    //     mappings.push(PolicyMapping::new(left, right));
    // }
    Ok((ret, PolicyMappings { mappings }))
}
