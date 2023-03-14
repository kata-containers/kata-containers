// Copyright (C) 2021 Scott Lamb <slamb@slamb.org>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Digest` authentication scheme, as in
//! [RFC 7616](https://datatracker.ietf.org/doc/html/rfc7616).

use std::{convert::TryFrom, fmt::Write as _, io::Write as _};

use digest::Digest;

use crate::{
    char_classes, ChallengeRef, ParamValue, PasswordParams, C_ATTR, C_ESCAPABLE, C_QDTEXT,
};

/// "Quality of protection" value.
///
/// The values here can be used in a bitmask as in [`DigestClient::qop`].
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum Qop {
    /// Authentication.
    Auth = 1,

    /// Authentication with integrity protection.
    ///
    /// "Integrity protection" means protection of the request entity body.
    AuthInt = 2,
}

impl Qop {
    /// Returns a string form as expected over the wire.
    fn as_str(self) -> &'static str {
        match self {
            Qop::Auth => "auth",
            Qop::AuthInt => "auth-int",
        }
    }
}

/// A set of zero or more [`Qop`]s.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct QopSet(u8);

impl std::fmt::Debug for QopSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut l = f.debug_set();
        if (self.0 & Qop::Auth as u8) != 0 {
            l.entry(&"auth");
        }
        if (self.0 & Qop::AuthInt as u8) != 0 {
            l.entry(&"auth-int");
        }
        l.finish()
    }
}

impl std::ops::BitAnd<Qop> for QopSet {
    type Output = bool;

    fn bitand(self, rhs: Qop) -> Self::Output {
        (self.0 & (rhs as u8)) != 0
    }
}

/// Client for a `Digest` challenge, as in [RFC 7616](https://datatracker.ietf.org/doc/html/rfc7616).
///
/// Most of the information here is taken from the `WWW-Authenticate` or
/// `Proxy-Authenticate` header. This also internally maintains a nonce counter.
///
/// ## Implementation notes
///
/// *   Recalculates `H(A1)` on each [`DigestClient::respond`] call. It'd be
///     more CPU-efficient to calculate `H(A1)` only once by supplying the
///     username and password at construction time or by caching (username,
///     password) -> `H(A1)` mappings internally. `DigestClient` prioritizes
///     simplicity instead.
/// *   There's no support yet for parsing the `Authentication-Info` and
///     `Proxy-Authentication-Info` header fields described by [RFC 7616 section
///     3.5](https://datatracker.ietf.org/doc/html/rfc7616#section-3.5).
///     PRs welcome!
/// *   Always responds using `UTF-8`, and thus doesn't use or keep around the `charset`
///     parameter. The RFC only allows that parameter to be set to `UTF-8` anyway.
/// *   Supports [RFC 2069](https://datatracker.ietf.org/doc/html/rfc2069) compatibility as in
///     [RFC 2617 section 3.2.2.1](https://datatracker.ietf.org/doc/html/rfc2617#section-3.2.2.1),
///     even though RFC 7616 drops it. There are still RTSP cameras being sold
///     in 2021 that use the RFC 2069-style calculations.
/// *   Supports RFC 7616 `userhash`, even though it seems impractical and only
///     marginally useful. The server must index the userhash for each supported
///     algorithm or calculate it on-the-fly for all users in the database.
/// *   The `-sess` algorithm variants haven't been tested; there's no example
///     in the RFCs.
///
/// ## Security considerations
///
/// We strongly advise *servers* against implementing `Digest`:
///
/// *   It's actively harmful in that it prevents the server from securing their
///     password storage via salted password hashes. See [RFC 7616 Section
///     5.2](https://datatracker.ietf.org/doc/html/rfc7616#section-5.2).
///     When your server offers `Digest` authentication, it is advertising that
///     it stores plaintext passwords!
/// *   It's no replacement for TLS in terms of protecting confidentiality of
///     the password, much less confidentiality of any other information.
///
/// For *clients*, when a server supports both `Digest` and `Basic`, we advise
/// using `Digest`. It provides (slightly) more confidentiality of passwords
/// over the wire.
///
/// Some servers *only* support `Digest`. E.g.,
/// [ONVIF](https://www.onvif.org/profiles/specifications/) mandates the
/// `Digest` scheme. It doesn't prohibit implementing other schemes, but some
/// cameras meet the specification's requirement and do no more.
#[derive(Eq, PartialEq)]
pub struct DigestClient {
    /// Holds unescaped versions of all string fields.
    ///
    /// Using a single `String` minimizes the size of the `DigestClient`
    /// itself and/or any option/enum it may be wrapped in. It also minimizes
    /// padding bytes after each allocation. The fields as stored as follows:
    ///
    /// 1.  `realm`: `[0, domain_start)`
    /// 2.  `domain`: `[domain_start, opaque_start)`
    /// 3.  `opaque`: `[opaque_start, nonce_start)`
    /// 4.  `nonce`: `[nonce_start, buf.len())`
    buf: Box<str>,

    // Positions described in `buf` comment above. See respective methods' doc
    // comments for more information. These are stored as `u16` to save space,
    // and because it's unreasonable for them to be large.
    domain_start: u16,
    opaque_start: u16,
    nonce_start: u16,

    // Non-string fields. See respective methods' doc comments for more information.
    algorithm: Algorithm,
    session: bool,
    stale: bool,
    rfc2069_compat: bool,
    userhash: bool,
    qop: QopSet,
    nc: u32,
}

impl DigestClient {
    /// Returns a string to be displayed to users so they know which username
    /// and password to use.
    ///
    /// This string should contain at least the name of
    /// the host performing the authentication and might additionally
    /// indicate the collection of users who might have access.  An
    /// example is `registered_users@example.com`.  (See [Section 2.2 of
    /// RFC 7235](https://datatracker.ietf.org/doc/html/rfc7235#section-2.2) for
    /// more details.)
    #[inline]
    pub fn realm(&self) -> &str {
        &self.buf[..self.domain_start as usize]
    }

    /// Returns the domain, a space-separated list of URIs, as specified in RFC
    /// 3986, that define the protection space.
    ///
    /// If the domain parameter is absent, returns an empty string, which is semantically
    /// identical according to the RFC.
    #[inline]
    pub fn domain(&self) -> &str {
        &self.buf[self.domain_start as usize..self.opaque_start as usize]
    }

    /// Returns the nonce, a server-specified string which should be uniquely
    /// generated each time a 401 response is made.
    #[inline]
    pub fn nonce(&self) -> &str {
        &self.buf[self.nonce_start as usize..]
    }

    /// Returns string of data, specified by the server, that SHOULD be returned
    /// by the client unchanged in the Authorization header field of subsequent
    /// requests with URIs in the same protection space.
    ///
    /// Currently an empty `opaque` is treated as an absent one.
    #[inline]
    pub fn opaque(&self) -> Option<&str> {
        if self.opaque_start == self.nonce_start {
            None
        } else {
            Some(&self.buf[self.opaque_start as usize..self.nonce_start as usize])
        }
    }

    /// Returns a flag indicating that the previous request from the client was
    /// rejected because the nonce value was stale.
    #[inline]
    pub fn stale(&self) -> bool {
        self.stale
    }

    /// Returns true if using [RFC 2069](https://datatracker.ietf.org/doc/html/rfc2069)
    /// compatibility mode as in [RFC 2617 section
    /// 3.2.2.1](https://datatracker.ietf.org/doc/html/rfc2617#section-3.2.2.1).
    ///
    /// If so, `request-digest` is calculated without the nonce count, conce, or qop.
    #[inline]
    pub fn rfc2069_compat(&self) -> bool {
        self.rfc2069_compat
    }

    /// Returns the algorithm used to produce the digest and an unkeyed digest.
    #[inline]
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    /// Returns if the session style `A1` will be used.
    #[inline]
    pub fn session(&self) -> bool {
        self.session
    }

    /// Returns the acceptable `qop` (quality of protection) values.
    #[inline]
    pub fn qop(&self) -> QopSet {
        self.qop
    }

    /// Returns the number of times the server-supplied nonce has been used by
    /// [`DigestClient::respond`].
    #[inline]
    pub fn nonce_count(&self) -> u32 {
        self.nc
    }

    /// Responds to the challenge with the supplied parameters.
    ///
    /// The caller should use the returned string as an `Authorization` or
    /// `Proxy-Authorization` header value.
    #[inline]
    pub fn respond(&mut self, p: &PasswordParams) -> Result<String, String> {
        self.respond_inner(p, &new_random_cnonce())
    }

    /// Responds using a fixed cnonce **for testing only**.
    ///
    /// In production code, use [`DigestClient::respond`] instead, which generates a new
    /// random cnonce value.
    #[inline]
    pub fn respond_with_testing_cnonce(
        &mut self,
        p: &PasswordParams,
        cnonce: &str,
    ) -> Result<String, String> {
        self.respond_inner(p, cnonce)
    }

    /// Helper for respond methods.
    ///
    /// We don't simply implement this as `respond_with_testing_cnonce` and have
    /// `respond` delegate to that method because it'd be confusing/alarming if
    /// that method name ever shows up in production stack traces.
    /// and have `respond` delegate to the testing version. We don't do that because
    fn respond_inner(&mut self, p: &PasswordParams, cnonce: &str) -> Result<String, String> {
        let realm = self.realm();
        let mut h_a1 = self.algorithm.h(&[
            p.username.as_bytes(),
            b":",
            realm.as_bytes(),
            b":",
            p.password.as_bytes(),
        ]);
        if self.session {
            h_a1 = self.algorithm.h(&[
                h_a1.as_bytes(),
                b":",
                self.nonce().as_bytes(),
                b":",
                cnonce.as_bytes(),
            ]);
        }

        // Select the best available qop and calculate H(A2) as in
        // [https://datatracker.ietf.org/doc/html/rfc7616#section-3.4.3].
        let (h_a2, qop);
        if let (Some(body), true) = (p.body, self.qop & Qop::AuthInt) {
            h_a2 = self
                .algorithm
                .h(&[p.method.as_bytes(), b":", p.uri.as_bytes(), b":", body]);
            qop = Qop::AuthInt;
        } else if self.qop & Qop::Auth {
            h_a2 = self
                .algorithm
                .h(&[p.method.as_bytes(), b":", p.uri.as_bytes()]);
            qop = Qop::Auth;
        } else {
            return Err("no supported/available qop".into());
        }

        let nc = self.nc.checked_add(1).ok_or("nonce count exhausted")?;
        let mut hex_nc = [0u8; 8];
        let _ = write!(&mut hex_nc[..], "{:08x}", nc);
        let str_hex_nc = match std::str::from_utf8(&hex_nc[..]) {
            Ok(h) => h,
            Err(_) => unreachable!(),
        };

        // https://datatracker.ietf.org/doc/html/rfc2617#section-3.2.2.1
        let response = if self.rfc2069_compat {
            self.algorithm.h(&[
                h_a1.as_bytes(),
                b":",
                self.nonce().as_bytes(),
                b":",
                h_a2.as_bytes(),
            ])
        } else {
            self.algorithm.h(&[
                h_a1.as_bytes(),
                b":",
                self.nonce().as_bytes(),
                b":",
                &hex_nc[..],
                b":",
                cnonce.as_bytes(),
                b":",
                qop.as_str().as_bytes(),
                b":",
                h_a2.as_bytes(),
            ])
        };

        let mut out = String::with_capacity(128);
        out.push_str("Digest ");
        if self.userhash {
            let hashed = self
                .algorithm
                .h(&[p.username.as_bytes(), b":", realm.as_bytes()]);
            append_quoted_key_value(&mut out, "username", &hashed)?;
            append_unquoted_key_value(&mut out, "userhash", "true");
        } else if is_valid_quoted_value(p.username) {
            append_quoted_key_value(&mut out, "username", p.username)?;
        } else {
            append_extended_key_value(&mut out, "username", p.username);
        }
        append_quoted_key_value(&mut out, "realm", self.realm())?;
        append_quoted_key_value(&mut out, "uri", p.uri)?;
        append_quoted_key_value(&mut out, "nonce", self.nonce())?;
        if !self.rfc2069_compat {
            append_unquoted_key_value(&mut out, "algorithm", self.algorithm.as_str(self.session));
            append_unquoted_key_value(&mut out, "nc", str_hex_nc);
            append_quoted_key_value(&mut out, "cnonce", cnonce)?;
            append_unquoted_key_value(&mut out, "qop", qop.as_str());
        }
        append_quoted_key_value(&mut out, "response", &response)?;
        if let Some(o) = self.opaque() {
            append_quoted_key_value(&mut out, "opaque", o)?;
        }
        out.truncate(out.len() - 2); // remove final ", "
        self.nc = nc;
        Ok(out)
    }
}

impl TryFrom<&ChallengeRef<'_>> for DigestClient {
    type Error = String;

    fn try_from(value: &ChallengeRef<'_>) -> Result<Self, Self::Error> {
        if !value.scheme.eq_ignore_ascii_case("Digest") {
            return Err(format!(
                "DigestClientContext doesn't support challenge scheme {:?}",
                value.scheme
            ));
        }
        let mut buf_len = 0;
        let mut unused_len = 0;
        let mut realm = None;
        let mut domain = None;
        let mut nonce = None;
        let mut opaque = None;
        let mut stale = false;
        let mut algorithm_and_session = None;
        let mut qop_str = None;
        let mut userhash_str = None;

        // Parse response header field parameters as in
        // [https://datatracker.ietf.org/doc/html/rfc7616#section-3.3].
        for (k, v) in &value.params {
            // Note that "stale" and "algorithm" can be directly compared
            // without unescaping because RFC 7616 section 3.3 says "For
            // historical reasons, a sender MUST NOT generate the quoted string
            // syntax values for the following parameters: stale and algorithm."
            if store_param(k, v, "realm", &mut realm, &mut buf_len)?
                || store_param(k, v, "domain", &mut domain, &mut buf_len)?
                || store_param(k, v, "nonce", &mut nonce, &mut buf_len)?
                || store_param(k, v, "opaque", &mut opaque, &mut buf_len)?
                || store_param(k, v, "qop", &mut qop_str, &mut unused_len)?
                || store_param(k, v, "userhash", &mut userhash_str, &mut unused_len)?
            {
                // Do nothing here.
            } else if k.eq_ignore_ascii_case("stale") {
                stale = v.escaped.eq_ignore_ascii_case("true");
            } else if k.eq_ignore_ascii_case("algorithm") {
                algorithm_and_session = Some(Algorithm::parse(v.escaped)?);
            }
        }
        let realm = realm.ok_or("missing required parameter realm")?;
        let nonce = nonce.ok_or("missing required parameter nonce")?;
        if buf_len > u16::MAX as usize {
            // Incredibly unlikely, but just for completeness.
            return Err(format!(
                "Unescaped parameters' length {} exceeds u16::MAX!",
                buf_len
            ));
        }

        let algorithm_and_session = algorithm_and_session.unwrap_or((Algorithm::Md5, false));

        let mut buf = String::with_capacity(buf_len);
        let mut qop = QopSet(0);
        let rfc2069_compat = if let Some(qop_str) = qop_str {
            let qop_str = qop_str.unescaped_with_scratch(&mut buf);
            for v in qop_str.split(',') {
                let v = v.trim();
                if v.eq_ignore_ascii_case("auth") {
                    qop.0 |= Qop::Auth as u8;
                } else if v.eq_ignore_ascii_case("auth-int") {
                    qop.0 |= Qop::AuthInt as u8;
                }
            }
            if qop.0 == 0 {
                return Err(format!("no supported qop in {:?}", qop_str));
            }
            buf.clear();
            false
        } else {
            // An absent qop is treated as "auth", according to
            // https://datatracker.ietf.org/doc/html/rfc7616#section-3.4.3
            qop.0 |= Qop::Auth as u8;
            true
        };
        let userhash;
        if let Some(userhash_str) = userhash_str {
            let userhash_str = userhash_str.unescaped_with_scratch(&mut buf);
            userhash = userhash_str.eq_ignore_ascii_case("true");
            buf.clear();
        } else {
            userhash = false;
        };
        realm.append_unescaped(&mut buf);
        let domain_start = buf.len();
        if let Some(d) = domain {
            d.append_unescaped(&mut buf);
        }
        let opaque_start = buf.len();
        if let Some(o) = opaque {
            o.append_unescaped(&mut buf);
        }
        let nonce_start = buf.len();
        nonce.append_unescaped(&mut buf);
        Ok(DigestClient {
            buf: buf.into_boxed_str(),
            domain_start: domain_start as u16,
            opaque_start: opaque_start as u16,
            nonce_start: nonce_start as u16,
            algorithm: algorithm_and_session.0,
            session: algorithm_and_session.1,
            stale,
            rfc2069_compat,
            userhash,
            qop,
            nc: 0,
        })
    }
}

impl std::fmt::Debug for DigestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DigestClient")
            .field("realm", &self.realm())
            .field("domain", &self.domain())
            .field("opaque", &self.opaque())
            .field("nonce", &self.nonce())
            .field("algorithm", &self.algorithm.as_str(self.session))
            .field("stale", &self.stale)
            .field("qop", &self.qop)
            .field("rfc2069_compat", &self.rfc2069_compat)
            .field("userhash", &self.userhash)
            .field("nc", &self.nc)
            .finish()
    }
}

/// Helper for `DigestClient::try_from` which stashes away a `&ParamValue`.
fn store_param<'v, 'tmp>(
    k: &'tmp str,
    v: &'v ParamValue<'v>,
    expected_k: &'tmp str,
    set_v: &'tmp mut Option<&'v ParamValue<'v>>,
    add_len: &'tmp mut usize,
) -> Result<bool, String> {
    if !k.eq_ignore_ascii_case(expected_k) {
        return Ok(false);
    }
    if set_v.is_some() {
        return Err(format!("duplicate parameter {:?}", k));
    }
    *add_len += v.unescaped_len();
    *set_v = Some(v);
    Ok(true)
}

fn is_valid_quoted_value(s: &str) -> bool {
    for &b in s.as_bytes() {
        if char_classes(b) & (C_QDTEXT | C_ESCAPABLE) == 0 {
            return false;
        }
    }
    true
}

fn append_extended_key_value(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str("*=UTF-8''");
    for &b in value.as_bytes() {
        if (char_classes(b) & C_ATTR) != 0 {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{:02X}", b);
        }
    }
    out.push_str(", ");
}

fn append_unquoted_key_value(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    out.push_str(value);
    out.push_str(", ");
}

fn append_quoted_key_value(out: &mut String, key: &str, value: &str) -> Result<(), String> {
    out.push_str(key);
    out.push_str("=\"");
    let mut first_unwritten = 0;
    let bytes = value.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        // Note that bytes >= 128 are in neither C_QDTEXT nor C_ESCAPABLE, so every allowed byte
        // is a full character.
        let class = char_classes(b);
        if (class & C_QDTEXT) != 0 {
            // Just advance.
        } else if (class & C_ESCAPABLE) != 0 {
            out.push_str(&value[first_unwritten..i]);
            out.push('\\');
            out.push(char::from(b));
            first_unwritten = i + 1;
        } else {
            return Err(format!("invalid {} value {:?}", key, value));
        }
    }
    out.push_str(&value[first_unwritten..]);
    out.push_str("\", ");
    Ok(())
}

/// Supported algorithm from the [HTTP Digest Algorithm Values
/// registry](https://www.iana.org/assignments/http-dig-alg/http-dig-alg.xhtml).
///
/// This doesn't store whether the session variant (`<Algorithm>-sess`) was
/// requested; see [`DigestClient::session`] for that.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Algorithm {
    Md5,
    Sha256,
    Sha512Trunc256,
}

impl Algorithm {
    /// Parses a string into a tuple of `Algorithm` and a bool representing
    /// whether the `-sess` suffix is present.
    fn parse(s: &str) -> Result<(Self, bool), String> {
        Ok(match s {
            "MD5" => (Algorithm::Md5, false),
            "MD5-sess" => (Algorithm::Md5, true),
            "SHA-256" => (Algorithm::Sha256, false),
            "SHA-256-sess" => (Algorithm::Sha256, true),
            "SHA-512-256" => (Algorithm::Sha512Trunc256, false),
            "SHA-512-256-sess" => (Algorithm::Sha512Trunc256, true),
            _ => return Err(format!("unknown algorithm {:?}", s)),
        })
    }

    fn as_str(&self, session: bool) -> &'static str {
        match (self, session) {
            (Algorithm::Md5, false) => "MD5",
            (Algorithm::Md5, true) => "MD5-sess",
            (Algorithm::Sha256, false) => "SHA-256",
            (Algorithm::Sha256, true) => "SHA-256-sess",
            (Algorithm::Sha512Trunc256, false) => "SHA-512-256",
            (Algorithm::Sha512Trunc256, true) => "SHA-512-256-sess",
        }
    }

    fn h(&self, items: &[&[u8]]) -> String {
        match self {
            Algorithm::Md5 => h(md5::Md5::new(), items),
            Algorithm::Sha256 => h(sha2::Sha256::new(), items),
            Algorithm::Sha512Trunc256 => h(sha2::Sha512_256::new(), items),
        }
    }
}

fn h<D: Digest>(mut d: D, items: &[&[u8]]) -> String {
    for i in items {
        d.update(i);
    }
    hex::encode(d.finalize())
}

fn new_random_cnonce() -> String {
    let raw: [u8; 16] = rand::random();
    hex::encode(&raw[..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Tests the example from [RFC 7616 section 3.9.1: SHA-256 and
    /// MD5](https://datatracker.ietf.org/doc/html/rfc7616#section-3.9.1).
    #[test]
    fn sha256_and_md5() {
        let www_authenticate = "\
            Digest \
            realm=\"http-auth@example.org\", \
            qop=\"auth, auth-int\", \
            algorithm=SHA-256, \
            nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
            opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\", \
            Digest \
            realm=\"http-auth@example.org\", \
            qop=\"auth, auth-int\", \
            algorithm=MD5, \
            nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
            opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\"";
        let challenges = dbg!(crate::parse_challenges(www_authenticate).unwrap());
        assert_eq!(challenges.len(), 2);
        let ctxs: Result<Vec<_>, _> = challenges.iter().map(DigestClient::try_from).collect();
        let mut ctxs = dbg!(ctxs.unwrap());
        assert_eq!(ctxs[1].realm(), "http-auth@example.org");
        assert_eq!(ctxs[1].domain(), "");
        assert_eq!(
            ctxs[1].nonce(),
            "7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v"
        );
        assert_eq!(
            ctxs[1].opaque(),
            Some("FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS")
        );
        assert_eq!(ctxs[1].stale(), false);
        assert_eq!(ctxs[1].algorithm(), Algorithm::Md5);
        assert_eq!(ctxs[1].qop().0, (Qop::Auth as u8) | (Qop::AuthInt as u8));
        assert_eq!(ctxs[1].nonce_count(), 0);
        let params = crate::PasswordParams {
            username: "Mufasa",
            password: "Circle of Life",
            uri: "/dir/index.html",
            body: None,
            method: "GET",
        };
        assert_eq!(
            &mut ctxs[0]
                .respond_with_testing_cnonce(
                    &params,
                    "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ"
                )
                .unwrap(),
            "Digest username=\"Mufasa\", \
                    realm=\"http-auth@example.org\", \
                    uri=\"/dir/index.html\", \
                    nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
                    algorithm=SHA-256, \
                    nc=00000001, \
                    cnonce=\"f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ\", \
                    qop=auth, \
                    response=\"753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1\", \
                    opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\""
        );
        assert_eq!(ctxs[0].nc, 1);
        assert_eq!(
            &mut ctxs[1]
                .respond_with_testing_cnonce(
                    &params,
                    "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ"
                )
                .unwrap(),
            "Digest username=\"Mufasa\", \
                    realm=\"http-auth@example.org\", \
                    uri=\"/dir/index.html\", \
                    nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
                    algorithm=MD5, \
                    nc=00000001, \
                    cnonce=\"f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ\", \
                    qop=auth, \
                    response=\"8ca523f5e9506fed4657c9700eebdbec\", \
                    opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\""
        );
        assert_eq!(ctxs[1].nc, 1);
    }

    /// Tests a made-up example with `MD5-sess`. There's no example in the RFC,
    /// and these values haven't been tested against any other implementation.
    /// But having the test here ensures we don't accidentally change the
    /// algorithm.
    #[test]
    fn md5_sess() {
        let www_authenticate = "\
            Digest \
            realm=\"http-auth@example.org\", \
            qop=\"auth, auth-int\", \
            algorithm=MD5-sess, \
            nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
            opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\"";
        let challenges = dbg!(crate::parse_challenges(www_authenticate).unwrap());
        assert_eq!(challenges.len(), 1);
        let ctxs: Result<Vec<_>, _> = challenges.iter().map(DigestClient::try_from).collect();
        let mut ctxs = dbg!(ctxs.unwrap());
        assert_eq!(ctxs[0].realm(), "http-auth@example.org");
        assert_eq!(ctxs[0].domain(), "");
        assert_eq!(
            ctxs[0].nonce(),
            "7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v"
        );
        assert_eq!(
            ctxs[0].opaque(),
            Some("FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS")
        );
        assert_eq!(ctxs[0].stale(), false);
        assert_eq!(ctxs[0].algorithm(), Algorithm::Md5);
        assert_eq!(ctxs[0].session(), true);
        assert_eq!(ctxs[0].qop().0, (Qop::Auth as u8) | (Qop::AuthInt as u8));
        assert_eq!(ctxs[0].nonce_count(), 0);
        let params = crate::PasswordParams {
            username: "Mufasa",
            password: "Circle of Life",
            uri: "/dir/index.html",
            body: None,
            method: "GET",
        };
        assert_eq!(
            &mut ctxs[0]
                .respond_with_testing_cnonce(
                    &params,
                    "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ"
                )
                .unwrap(),
            "Digest username=\"Mufasa\", \
                    realm=\"http-auth@example.org\", \
                    uri=\"/dir/index.html\", \
                    nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
                    algorithm=MD5-sess, \
                    nc=00000001, \
                    cnonce=\"f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ\", \
                    qop=auth, \
                    response=\"e783283f46242139c486a698fec7211d\", \
                    opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\""
        );
        assert_eq!(ctxs[0].nc, 1);
    }

    /// Tests the example from [RFC 7616 section 3.9.2: SHA-512-256, Charset, and
    /// Userhash](https://datatracker.ietf.org/doc/html/rfc7616#section-3.9.2).
    #[test]
    fn sha512_256_charset() {
        let www_authenticate = "\
            Digest \
            realm=\"api@example.org\", \
            qop=\"auth\", \
            algorithm=SHA-512-256, \
            nonce=\"5TsQWLVdgBdmrQ0XsxbDODV+57QdFR34I9HAbC/RVvkK\", \
            opaque=\"HRPCssKJSGjCrkzDg8OhwpzCiGPChXYjwrI2QmXDnsOS\", \
            charset=UTF-8, \
            userhash=true";
        let challenges = dbg!(crate::parse_challenges(www_authenticate).unwrap());
        assert_eq!(challenges.len(), 1);
        let ctxs: Result<Vec<_>, _> = challenges.iter().map(DigestClient::try_from).collect();
        let mut ctxs = dbg!(ctxs.unwrap());
        assert_eq!(ctxs.len(), 1);
        assert_eq!(ctxs[0].realm(), "api@example.org");
        assert_eq!(ctxs[0].domain(), "");
        assert_eq!(
            ctxs[0].nonce(),
            "5TsQWLVdgBdmrQ0XsxbDODV+57QdFR34I9HAbC/RVvkK"
        );
        assert_eq!(
            ctxs[0].opaque(),
            Some("HRPCssKJSGjCrkzDg8OhwpzCiGPChXYjwrI2QmXDnsOS")
        );
        assert_eq!(ctxs[0].stale, false);
        assert_eq!(ctxs[0].userhash, true);
        assert_eq!(ctxs[0].algorithm, Algorithm::Sha512Trunc256);
        assert_eq!(ctxs[0].qop.0, Qop::Auth as u8);
        assert_eq!(ctxs[0].nc, 0);
        let params = crate::PasswordParams {
            username: "J\u{E4}s\u{F8}n Doe",
            password: "Secret, or not?",
            uri: "/doe.json",
            body: None,
            method: "GET",
        };

        // Note the username and response values in the RFC are *wrong*!
        // https://www.rfc-editor.org/errata/eid4897
        assert_eq!(
            &mut ctxs[0]
                .respond_with_testing_cnonce(
                    &params,
                    "NTg6RKcb9boFIAS3KrFK9BGeh+iDa/sm6jUMp2wds69v"
                )
                .unwrap(),
            "\
            Digest \
            username=\"793263caabb707a56211940d90411ea4a575adeccb7e360aeb624ed06ece9b0b\", \
            userhash=true, \
            realm=\"api@example.org\", \
            uri=\"/doe.json\", \
            nonce=\"5TsQWLVdgBdmrQ0XsxbDODV+57QdFR34I9HAbC/RVvkK\", \
            algorithm=SHA-512-256, \
            nc=00000001, \
            cnonce=\"NTg6RKcb9boFIAS3KrFK9BGeh+iDa/sm6jUMp2wds69v\", \
            qop=auth, \
            response=\"3798d4131c277846293534c3edc11bd8a5e4cdcbff78b05db9d95eeb1cec68a5\", \
            opaque=\"HRPCssKJSGjCrkzDg8OhwpzCiGPChXYjwrI2QmXDnsOS\""
        );
        assert_eq!(ctxs[0].nc, 1);
        ctxs[0].userhash = false;
        ctxs[0].nc = 0;
        assert_eq!(
            &mut ctxs[0]
                .respond_with_testing_cnonce(
                    &params,
                    "NTg6RKcb9boFIAS3KrFK9BGeh+iDa/sm6jUMp2wds69v"
                )
                .unwrap(),
            "\
            Digest \
            username*=UTF-8''J%C3%A4s%C3%B8n%20Doe, \
            realm=\"api@example.org\", \
            uri=\"/doe.json\", \
            nonce=\"5TsQWLVdgBdmrQ0XsxbDODV+57QdFR34I9HAbC/RVvkK\", \
            algorithm=SHA-512-256, \
            nc=00000001, \
            cnonce=\"NTg6RKcb9boFIAS3KrFK9BGeh+iDa/sm6jUMp2wds69v\", \
            qop=auth, \
            response=\"3798d4131c277846293534c3edc11bd8a5e4cdcbff78b05db9d95eeb1cec68a5\", \
            opaque=\"HRPCssKJSGjCrkzDg8OhwpzCiGPChXYjwrI2QmXDnsOS\""
        );
        assert_eq!(ctxs[0].nc, 1);
    }

    #[test]
    fn rfc2069() {
        // https://datatracker.ietf.org/doc/html/rfc2069#section-2.4
        // The response there is wrong! See https://www.rfc-editor.org/errata/eid749
        let www_authenticate = "\
            Digest \
            realm=\"testrealm@host.com\", \
            nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\", \
            opaque=\"5ccc069c403ebaf9f0171e9517f40e41\"";
        let challenges = dbg!(crate::parse_challenges(www_authenticate).unwrap());
        assert_eq!(challenges.len(), 1);
        let ctxs: Result<Vec<_>, _> = challenges.iter().map(DigestClient::try_from).collect();
        let mut ctxs = dbg!(ctxs.unwrap());
        assert_eq!(ctxs.len(), 1);
        assert_eq!(ctxs[0].qop.0, Qop::Auth as u8);
        assert_eq!(ctxs[0].rfc2069_compat, true);
        let params = crate::PasswordParams {
            username: "Mufasa",
            password: "CircleOfLife",
            uri: "/dir/index.html",
            body: None,
            method: "GET",
        };
        assert_eq!(
            &mut ctxs[0]
                .respond_with_testing_cnonce(&params, "unused")
                .unwrap(),
            "\
            Digest \
            username=\"Mufasa\", \
            realm=\"testrealm@host.com\", \
            uri=\"/dir/index.html\", \
            nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\", \
            response=\"1949323746fe6a43ef61f9606e7febea\", \
            opaque=\"5ccc069c403ebaf9f0171e9517f40e41\"",
        );
        assert_eq!(ctxs[0].nc, 1);
    }

    // See sizes with: cargo test -- --nocapture digest::tests::size
    #[test]
    fn size() {
        // This type should have a niche.
        assert_eq!(
            dbg!(std::mem::size_of::<DigestClient>()),
            dbg!(std::mem::size_of::<Option<DigestClient>>()),
        )
    }
}
