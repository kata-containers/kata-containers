[![crates.io](https://img.shields.io/crates/v/http-auth)](https://crates.io/crates/http-auth)
[![Released API docs](https://docs.rs/http-auth/badge.svg)](https://docs.rs/http-auth/)
[![CI](https://github.com/scottlamb/http-auth/workflows/CI/badge.svg)](https://github.com/scottlamb/http-auth/actions?query=workflow%3ACI)

Rust library for HTTP authentication. Parses challenge lists, responds
to `Basic` and `Digest` challenges. Likely to be extended with server
support and additional auth schemes.

HTTP authentication is described in the following documents and specifications:

*   [MDN documentation](https://developer.mozilla.org/en-US/docs/Web/HTTP/Authentication).
*   [RFC 7235](https://datatracker.ietf.org/doc/html/rfc7235):
    Hypertext Transfer Protocol (HTTP/1.1): Authentication.
*   [RFC 7617](https://datatracker.ietf.org/doc/html/rfc7617):
    The 'Basic' HTTP Authentication Scheme
*   [RFC 7616](https://datatracker.ietf.org/doc/html/rfc7616):
    HTTP Digest Access Authentication

This framework is primarily used with HTTP, as suggested by the name. It is
also used by some other protocols such as RTSP.

## Status

Young but well-tested. The API may change to improve ergonomics and
functionality. New functionality is likely to be added. PRs welcome!

## Goals

In order:

1.  **sound.** Currently no `unsafe` blocks in `http-auth` itself. All
    dependencies are common, trusted crates.
3.  **correct.** Precisely implements the specifications except where noted.
    Fuzz tests verify the hand-written parser never panics and matches a
    nom-based reference implementation.
4.  **light-weight.** Minimal dependencies; uses Cargo features so callers can
    avoid them when undesired. Simple code that minimizes monomorphization
    bloat. Small data structures; eg `http_auth::DigestClient` currently weighs
    in at 32 bytes plus one allocation for all string fields.
6.  **complete.** Implements both parsing and responding to challenges.
    (Currently only supports the client side and responding to the most common
    `Basic` and `Digest` schemes; future expansion is likely.)
7.  **ergonomic.** Creating a client for responding to a password challenge is
    a one-liner from a string header or a
    [`http::header::GetAll`](https://docs.rs/http/0.2.5/http/header/struct.GetAll.html).
8.  **fast enough.** HTTP authentication is a small part of a real program, and
    `http-auth`'s CPU usage should never be noticeable. For `Digest`'s
    cryptographic operations, it uses popular optimized crates. In other
    respects, `http-auth` is likely at least as efficient as other HTTP
    authentication crates, although I have no reason to believe their
    performance is problematic.

## Why a new crate?

There are at least a couple other available crates relating to HTTP
authentication. You may prefer them. Here's why `http-auth`'s author decided
not to use them.

### [`www-authenticate`](https://crates.io/crates/www-authenticate)

*   sound: `www-authenticate` has some unsound `transmute`s to static lifetime.
    (These likely aren't hard to fix though.)
*   light-weight: `www-authenticate` depends on `hyperx` and `unicase`, large
    dependencies which many useful programs don't include.
*   complete: `www-authenticate` only supports parsing of challenge lists, not
    responding to them.

### [`digest_auth`](https://crates.io/crates/digest_auth)

*   complete: `digest_auth` only supports `Digest`. It can't parse multiple
    challenges and will fail if given a list that starts with another scheme.
    Thus, if the server follows the advice of
    [RFC 7235 section 2.1](https://datatracker.ietf.org/doc/html/rfc7235) and
    lists another scheme such as `Basic` first, `digest_auth`'s parsing is
    insufficient.

### `www-authenticate` + `digest_auth` together

In addition to the `www-authenticate` caveats above, responding to password
challenges by using both `www-authenticate` and `digest_auth` is not complete
and ergonomic. The caller must do extra work:

*    explicitly consider both `Digest` and `Basic`, rather than using the
     abstract `http_auth::PasswordClient` that chooses the challenge for you.
*    when responding to a `Digest` challenge, construct a matching
     `digest_auth::WwwAuthenticateHeader` from the
     `www_authenticate::DigestChallenge`.
*    when responding to a `Basic` challenge, do the encoding manually.

## Author

Scott Lamb &lt;slamb@slamb.org>

## License

SPDX-License-Identifier: [MIT](https://spdx.org/licenses/MIT.html) OR [Apache-2.0](https://spdx.org/licenses/Apache-2.0.html)

See [LICENSE-MIT.txt](LICENSE-MIT.txt) or [LICENSE-APACHE](LICENSE-APACHE.txt),
respectively.
