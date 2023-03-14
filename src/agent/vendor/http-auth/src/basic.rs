// Copyright (C) 2021 Scott Lamb <slamb@slamb.org>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Basic` authentication scheme as in
//! [RFC 7617](https://datatracker.ietf.org/doc/html/rfc7617).

use std::convert::TryFrom;

use crate::ChallengeRef;

/// Encodes the given credentials.
///
/// This can be used to preemptively send `Basic` authentication, without
/// sending an unauthenticated request and waiting for a `401 Unauthorized`
/// response.
///
/// The caller should use the returned string as an `Authorization` or
/// `Proxy-Authorization` header value.
///
/// The caller is responsible for `username` and `password` being in the
/// correct format. Servers may expect arguments to be in Unicode
/// Normalization Form C as noted in [RFC 7617 section
/// 2.1](https://datatracker.ietf.org/doc/html/rfc7617#section-2.1).
///
/// ```rust
/// assert_eq!(
///     http_auth::basic::encode_credentials("Aladdin", "open sesame"),
///     "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==",
/// );
pub fn encode_credentials(username: &str, password: &str) -> String {
    let user_pass = format!("{}:{}", username, password);
    const PREFIX: &str = "Basic ";
    let mut value = String::with_capacity(PREFIX.len() + base64_encoded_len(user_pass.len()));
    value.push_str(PREFIX);
    base64::encode_config_buf(&user_pass[..], base64::STANDARD, &mut value);
    value
}

/// Returns the base64-encoded length for the given input length, including padding.
fn base64_encoded_len(input_len: usize) -> usize {
    (input_len + 2) / 3 * 4
}

/// Client for a `Basic` challenge, as in
/// [RFC 7617](https://datatracker.ietf.org/doc/html/rfc7617).
///
/// This implementation always uses `UTF-8`. Thus it doesn't use or store the
/// `charset` parameter, which the RFC only allows to be set to `UTF-8` anyway.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicClient {
    realm: Box<str>,
}

impl BasicClient {
    pub fn realm(&self) -> &str {
        &*self.realm
    }

    /// Responds to the challenge with the supplied parameters.
    ///
    /// This is functionally identical to [`encode_credentials`]; no parameters
    /// of the `BasicClient` are needed to produce the credentials.
    #[inline]
    pub fn respond(&self, username: &str, password: &str) -> String {
        encode_credentials(username, password)
    }
}

impl TryFrom<&ChallengeRef<'_>> for BasicClient {
    type Error = String;

    fn try_from(value: &ChallengeRef<'_>) -> Result<Self, Self::Error> {
        if !value.scheme.eq_ignore_ascii_case("Basic") {
            return Err(format!(
                "BasicClient doesn't support challenge scheme {:?}",
                value.scheme
            ));
        }
        let mut realm = None;
        for (k, v) in &value.params {
            if k.eq_ignore_ascii_case("realm") {
                realm = Some(v.to_unescaped());
            }
        }
        let realm = realm.ok_or("missing required parameter realm")?;
        Ok(BasicClient {
            realm: realm.into_boxed_str(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        // Example from https://datatracker.ietf.org/doc/html/rfc7617#section-2
        let ctx = BasicClient {
            realm: "WallyWorld".into(),
        };
        assert_eq!(
            ctx.respond("Aladdin", "open sesame"),
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );

        // Example from https://datatracker.ietf.org/doc/html/rfc7617#section-2.1
        // Note that this crate *always* uses UTF-8, not just when the server requests it.
        let ctx = BasicClient {
            realm: "foo".into(),
        };
        assert_eq!(ctx.respond("test", "123\u{A3}"), "Basic dGVzdDoxMjPCow==");
    }
}
