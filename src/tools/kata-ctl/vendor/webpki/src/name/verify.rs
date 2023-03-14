// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use super::{
    dns_name::{self, DnsNameRef},
    ip_address,
};
use crate::{
    cert::{Cert, EndEntityOrCa},
    der, Error,
};

pub fn verify_cert_dns_name(
    cert: &crate::EndEntityCert,
    dns_name: DnsNameRef,
) -> Result<(), Error> {
    let cert = cert.inner();
    let dns_name = untrusted::Input::from(dns_name.as_ref().as_ref());
    iterate_names(
        cert.subject,
        cert.subject_alt_name,
        Err(Error::CertNotValidForName),
        &|name| {
            match name {
                GeneralName::DnsName(presented_id) => {
                    match dns_name::presented_id_matches_reference_id(presented_id, dns_name) {
                        Some(true) => {
                            return NameIteration::Stop(Ok(()));
                        }
                        Some(false) => (),
                        None => {
                            return NameIteration::Stop(Err(Error::BadDer));
                        }
                    }
                }
                _ => (),
            }
            NameIteration::KeepGoing
        },
    )
}

// https://tools.ietf.org/html/rfc5280#section-4.2.1.10
pub fn check_name_constraints(
    input: Option<&mut untrusted::Reader>,
    subordinate_certs: &Cert,
) -> Result<(), Error> {
    let input = match input {
        Some(input) => input,
        None => {
            return Ok(());
        }
    };

    fn parse_subtrees<'b>(
        inner: &mut untrusted::Reader<'b>,
        subtrees_tag: der::Tag,
    ) -> Result<Option<untrusted::Input<'b>>, Error> {
        if !inner.peek(subtrees_tag.into()) {
            return Ok(None);
        }
        let subtrees = der::nested(inner, subtrees_tag, Error::BadDer, |tagged| {
            der::expect_tag_and_get_value(tagged, der::Tag::Sequence)
        })?;
        Ok(Some(subtrees))
    }

    let permitted_subtrees = parse_subtrees(input, der::Tag::ContextSpecificConstructed0)?;
    let excluded_subtrees = parse_subtrees(input, der::Tag::ContextSpecificConstructed1)?;

    let mut child = subordinate_certs;
    loop {
        iterate_names(child.subject, child.subject_alt_name, Ok(()), &|name| {
            check_presented_id_conforms_to_constraints(name, permitted_subtrees, excluded_subtrees)
        })?;

        child = match child.ee_or_ca {
            EndEntityOrCa::Ca(child_cert) => child_cert,
            EndEntityOrCa::EndEntity => {
                break;
            }
        };
    }

    Ok(())
}

fn check_presented_id_conforms_to_constraints(
    name: GeneralName,
    permitted_subtrees: Option<untrusted::Input>,
    excluded_subtrees: Option<untrusted::Input>,
) -> NameIteration {
    match check_presented_id_conforms_to_constraints_in_subtree(
        name,
        Subtrees::PermittedSubtrees,
        permitted_subtrees,
    ) {
        stop @ NameIteration::Stop(..) => {
            return stop;
        }
        NameIteration::KeepGoing => (),
    };

    check_presented_id_conforms_to_constraints_in_subtree(
        name,
        Subtrees::ExcludedSubtrees,
        excluded_subtrees,
    )
}

#[derive(Clone, Copy)]
enum Subtrees {
    PermittedSubtrees,
    ExcludedSubtrees,
}

fn check_presented_id_conforms_to_constraints_in_subtree(
    name: GeneralName,
    subtrees: Subtrees,
    constraints: Option<untrusted::Input>,
) -> NameIteration {
    let mut constraints = match constraints {
        Some(constraints) => untrusted::Reader::new(constraints),
        None => {
            return NameIteration::KeepGoing;
        }
    };

    let mut has_permitted_subtrees_match = false;
    let mut has_permitted_subtrees_mismatch = false;

    loop {
        // http://tools.ietf.org/html/rfc5280#section-4.2.1.10: "Within this
        // profile, the minimum and maximum fields are not used with any name
        // forms, thus, the minimum MUST be zero, and maximum MUST be absent."
        //
        // Since the default value isn't allowed to be encoded according to the
        // DER encoding rules for DEFAULT, this is equivalent to saying that
        // neither minimum or maximum must be encoded.
        fn general_subtree<'b>(
            input: &mut untrusted::Reader<'b>,
        ) -> Result<GeneralName<'b>, Error> {
            let general_subtree = der::expect_tag_and_get_value(input, der::Tag::Sequence)?;
            general_subtree.read_all(Error::BadDer, |subtree| general_name(subtree))
        }

        let base = match general_subtree(&mut constraints) {
            Ok(base) => base,
            Err(err) => {
                return NameIteration::Stop(Err(err));
            }
        };

        let matches = match (name, base) {
            (GeneralName::DnsName(name), GeneralName::DnsName(base)) => {
                dns_name::presented_id_matches_constraint(name, base).ok_or(Error::BadDer)
            }

            (GeneralName::DirectoryName(name), GeneralName::DirectoryName(base)) => Ok(
                presented_directory_name_matches_constraint(name, base, subtrees),
            ),

            (GeneralName::IpAddress(name), GeneralName::IpAddress(base)) => {
                ip_address::presented_id_matches_constraint(name, base)
            }

            // RFC 4280 says "If a name constraints extension that is marked as
            // critical imposes constraints on a particular name form, and an
            // instance of that name form appears in the subject field or
            // subjectAltName extension of a subsequent certificate, then the
            // application MUST either process the constraint or reject the
            // certificate." Later, the CABForum agreed to support non-critical
            // constraints, so it is important to reject the cert without
            // considering whether the name constraint it critical.
            (GeneralName::Unsupported(name_tag), GeneralName::Unsupported(base_tag))
                if name_tag == base_tag =>
            {
                Err(Error::NameConstraintViolation)
            }

            _ => Ok(false),
        };

        match (subtrees, matches) {
            (Subtrees::PermittedSubtrees, Ok(true)) => {
                has_permitted_subtrees_match = true;
            }

            (Subtrees::PermittedSubtrees, Ok(false)) => {
                has_permitted_subtrees_mismatch = true;
            }

            (Subtrees::ExcludedSubtrees, Ok(true)) => {
                return NameIteration::Stop(Err(Error::NameConstraintViolation));
            }

            (Subtrees::ExcludedSubtrees, Ok(false)) => (),

            (_, Err(err)) => {
                return NameIteration::Stop(Err(err));
            }
        }

        if constraints.at_end() {
            break;
        }
    }

    if has_permitted_subtrees_mismatch && !has_permitted_subtrees_match {
        // If there was any entry of the given type in permittedSubtrees, then
        // it required that at least one of them must match. Since none of them
        // did, we have a failure.
        NameIteration::Stop(Err(Error::NameConstraintViolation))
    } else {
        NameIteration::KeepGoing
    }
}

// TODO: document this.
fn presented_directory_name_matches_constraint(
    name: untrusted::Input,
    constraint: untrusted::Input,
    subtrees: Subtrees,
) -> bool {
    match subtrees {
        Subtrees::PermittedSubtrees => name == constraint,
        Subtrees::ExcludedSubtrees => true,
    }
}

#[derive(Clone, Copy)]
enum NameIteration {
    KeepGoing,
    Stop(Result<(), Error>),
}

fn iterate_names(
    subject: untrusted::Input,
    subject_alt_name: Option<untrusted::Input>,
    result_if_never_stopped_early: Result<(), Error>,
    f: &dyn Fn(GeneralName) -> NameIteration,
) -> Result<(), Error> {
    match subject_alt_name {
        Some(subject_alt_name) => {
            let mut subject_alt_name = untrusted::Reader::new(subject_alt_name);
            // https://bugzilla.mozilla.org/show_bug.cgi?id=1143085: An empty
            // subjectAltName is not legal, but some certificates have an empty
            // subjectAltName. Since we don't support CN-IDs, the certificate
            // will be rejected either way, but checking `at_end` before
            // attempting to parse the first entry allows us to return a better
            // error code.
            while !subject_alt_name.at_end() {
                let name = general_name(&mut subject_alt_name)?;
                match f(name) {
                    NameIteration::Stop(result) => {
                        return result;
                    }
                    NameIteration::KeepGoing => (),
                }
            }
        }
        None => (),
    }

    match f(GeneralName::DirectoryName(subject)) {
        NameIteration::Stop(result) => result,
        NameIteration::KeepGoing => result_if_never_stopped_early,
    }
}

// It is *not* valid to derive `Eq`, `PartialEq, etc. for this type. In
// particular, for the types of `GeneralName`s that we don't understand, we
// don't even store the value. Also, the meaning of a `GeneralName` in a name
// constraint is different than the meaning of the identically-represented
// `GeneralName` in other contexts.
#[derive(Clone, Copy)]
enum GeneralName<'a> {
    DnsName(untrusted::Input<'a>),
    DirectoryName(untrusted::Input<'a>),
    IpAddress(untrusted::Input<'a>),

    // The value is the `tag & ~(der::CONTEXT_SPECIFIC | der::CONSTRUCTED)` so
    // that the name constraint checking matches tags regardless of whether
    // those bits are set.
    Unsupported(u8),
}

fn general_name<'a>(input: &mut untrusted::Reader<'a>) -> Result<GeneralName<'a>, Error> {
    use ring::io::der::{CONSTRUCTED, CONTEXT_SPECIFIC};
    #[allow(clippy::identity_op)]
    const OTHER_NAME_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED | 0;
    const RFC822_NAME_TAG: u8 = CONTEXT_SPECIFIC | 1;
    const DNS_NAME_TAG: u8 = CONTEXT_SPECIFIC | 2;
    const X400_ADDRESS_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED | 3;
    const DIRECTORY_NAME_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED | 4;
    const EDI_PARTY_NAME_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED | 5;
    const UNIFORM_RESOURCE_IDENTIFIER_TAG: u8 = CONTEXT_SPECIFIC | 6;
    const IP_ADDRESS_TAG: u8 = CONTEXT_SPECIFIC | 7;
    const REGISTERED_ID_TAG: u8 = CONTEXT_SPECIFIC | 8;

    let (tag, value) = der::read_tag_and_get_value(input)?;
    let name = match tag {
        DNS_NAME_TAG => GeneralName::DnsName(value),
        DIRECTORY_NAME_TAG => GeneralName::DirectoryName(value),
        IP_ADDRESS_TAG => GeneralName::IpAddress(value),

        OTHER_NAME_TAG
        | RFC822_NAME_TAG
        | X400_ADDRESS_TAG
        | EDI_PARTY_NAME_TAG
        | UNIFORM_RESOURCE_IDENTIFIER_TAG
        | REGISTERED_ID_TAG => GeneralName::Unsupported(tag & !(CONTEXT_SPECIFIC | CONSTRUCTED)),

        _ => return Err(Error::BadDer),
    };
    Ok(name)
}
