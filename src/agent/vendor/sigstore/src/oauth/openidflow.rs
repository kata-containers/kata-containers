//
// Copyright 2022 The Sigstore Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This provides a method for retreiving a OpenID Connect ID Token and scope from the sigstore project.
//!
//! The main entry point is the [`OpenIDAuthorize::auth_url`](OpenIDAuthorize::auth_url) function.
//! This requires four parameters:
//! - `client_id`: the client ID of the application
//! - `client_secret`: the client secret of the application
//! - `issuer`: the URL of the OpenID Connect server
//! - `redirect_uri`: the URL of the callback endpoint
//!
//! The `auth_url` function returns the following:
//!
//! - `authorize_url` is a URL that can be opened in a browser. The user will be
//! prompted to login and authorize the application. The user will be redirected to
//! the `redirect_uri` URL with a code parameter.
//!
//! - `client` is a client object that can be used to make requests to the OpenID
//! Connect server.
//!
//! - `nonce` is a random value that is used to prevent replay attacks.
//!
//! - `pkce_verifier` is a PKCE verifier that can be used to generate the code_verifier
//! value.
//!
//! Once you have recieved the above tuple, you can use the [`RedirectListener::redirect_listener`](RedirectListener::redirect_listener)
//! function to get the ID Token and scope.
//!
//! The `redirect_listener` function requires the following parameters:
//! - `client_redirect_host`: the address for callback.
//! - `client`: the client object
//! - `nonce`: the nonce value
//! - `pkce_verifier`: the PKCE verifier
//!
//! The `IdTokenClaims` this contains params such as `email` and the `access_token`.
//!
//! It maybe prefered to instead develop your own listener. If so bypass using the
//! [`RedirectListener::redirect_listener`](RedirectListener::redirect_listener) function and
//! simply send the values retrieved from the [`OpenIDAuthorize::auth_url`](OpenIDAuthorize::auth_url)
//! to your own listener.
//!
//!
//! **Warning:** one of the dependencies of the [`OpenIDAuthorize::auth_url`](OpenIDAuthorize::auth_url) performs
//! blocking operations. Because of that it can cause panics at runtime if invoked inside of `async` code.
//! If you need to use this function inside of an async code you must wrap it inside of a `spawn_blocking` instruction:
//!
//! ```rust,ignore
//! use tokio::task::spawn_blocking;
//!
//! async fn my_async_function() {
//!    // ... your code
//!
//!    let oidc_url = spawn_blocking(||
//!     oauth::openidflow::OpenIDAuthorize::new(
//!       "sigstore",
//!       "",
//!       "https://oauth2.sigstore.dev/auth",
//!       "http://localhost:8080",
//!     )
//!     .auth_url()
//!    )
//!    .await
//!    .expect("Error spawning blocking task");
//!
//!    // ... your code
//! }
//! ```
//! This of course has a performance hit when used inside of an async function.

use crate::errors::{Result, SigstoreError};
use tracing::error;

use openidconnect::core::{
    CoreClient, CoreIdToken, CoreIdTokenClaims, CoreIdTokenVerifier, CoreProviderMetadata,
    CoreResponseType,
};
use openidconnect::reqwest::http_client;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use url::Url;

#[derive(Debug)]
pub struct OpenIDAuthorize {
    oidc_cliend_id: String,
    oidc_client_secret: String,
    oidc_issuer: String,
    redirect_url: String,
}

impl OpenIDAuthorize {
    //! Create a new OpenIDAuthorize struct
    //!
    //! # Arguments
    //!
    //! * `client_id` - the client ID of the application
    //! * `client_secret` - the client secret of the application
    //! * `issuer` - the URL of the OpenID Connect server
    //! * `redirect_url` - client redirect URL
    //! # Example
    //!
    //! ```rust,ignore
    //! use sigstore::oauth::openidflow::OpenIDAuthorize;
    //!
    //! let oidc = OpenIDAuthorize::new("client_id", "client_secret", "https://example.com", "http://localhost:8080").auth_url();
    //! ```
    pub fn new(client_id: &str, client_secret: &str, issuer: &str, redirect_url: &str) -> Self {
        Self {
            oidc_cliend_id: client_id.to_string(),
            oidc_client_secret: client_secret.to_string(),
            oidc_issuer: issuer.to_string(),
            redirect_url: redirect_url.to_string(),
        }
    }
    pub fn auth_url(&self) -> Result<(Url, CoreClient, Nonce, PkceCodeVerifier)> {
        let client_id = ClientId::new(self.oidc_cliend_id.to_owned());
        let client_secret = ClientSecret::new(self.oidc_client_secret.to_owned());
        let issuer = IssuerUrl::new(self.oidc_issuer.to_owned()).expect("Missing the OIDC_ISSUER.");

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let provider_metadata =
            CoreProviderMetadata::discover(&issuer, http_client).map_err(|err| {
                error!("Error is: {:?}", err);
                SigstoreError::ClaimsVerificationError
            })?;

        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                .set_redirect_uri(
                    RedirectUrl::new(self.redirect_url.to_owned()).expect("Invalid redirect URL"),
                );

        let (authorize_url, _, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();
        Ok((authorize_url, client, nonce, pkce_verifier))
    }
}

pub struct RedirectListener {
    client_redirect_host: String,
    client: CoreClient,
    nonce: Nonce,
    pkce_verifier: PkceCodeVerifier,
}

impl RedirectListener {
    //! Create a new RedirectListener struct
    //!
    //! # Arguments
    //!
    //! * `client_redirect_host` - The client callback host IP:PORT
    //! * `client` - CoreClient instance (returned from OpenIDAuthorize)
    //! * `nonce` - Nonce (returned from OpenIDAuthorize)
    //! * `pkce_verifier` - client redirect URL
    //! # Example
    //!
    //! ```rust,ignore
    //! use sigstore::oauth::openidflow::RedirectListener;
    //!
    //! let oidc = RedirectListener::new("127.0.0.1:8080", client, nonce, pkce_verifier).redirect_listener();
    //! ```
    pub fn new(
        client_redirect_host: &str,
        client: CoreClient,
        nonce: Nonce,
        pkce_verifier: PkceCodeVerifier,
    ) -> Self {
        Self {
            client_redirect_host: client_redirect_host.to_string(),
            client,
            nonce,
            pkce_verifier,
        }
    }
    pub fn redirect_listener(self) -> Result<(CoreIdTokenClaims, CoreIdToken)> {
        let listener = TcpListener::bind(self.client_redirect_host.clone())?;
        #[allow(clippy::manual_flatten)]
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let code;
                {
                    let mut reader = BufReader::new(&stream);

                    let mut request_line = String::new();
                    reader.read_line(&mut request_line)?;

                    let client_redirect_host = request_line
                        .split_whitespace()
                        .nth(1)
                        .ok_or(SigstoreError::RedirectUrlRequestLineError)?;
                    let url =
                        Url::parse(format!("http://localhost{}", client_redirect_host).as_str())?;

                    let code_pair = url
                        .query_pairs()
                        .find(|pair| {
                            let &(ref key, _) = pair;
                            key == "code"
                        })
                        .ok_or(SigstoreError::CodePairError)?;

                    let (_, value) = code_pair;
                    code = AuthorizationCode::new(value.into_owned());
                }

                let html_page = r#"<html>
                <title>Sigstore Auth</title>
                <body>
                <h1>Sigstore Auth Successful</h1>
                <p>You may now close this page.</p>
                </body>
                </html>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
                    html_page.len(),
                    html_page
                );
                stream.write_all(response.as_bytes())?;

                let token_response = self
                    .client
                    .exchange_code(code)
                    .set_pkce_verifier(self.pkce_verifier)
                    .request(http_client)
                    .map_err(|_| SigstoreError::ClaimsAccessPointError)?;

                let id_token = token_response
                    .extra_fields()
                    .id_token()
                    .ok_or(SigstoreError::NoIDToken)?;

                let id_token_verifier: CoreIdTokenVerifier = self.client.id_token_verifier();

                let id_token_claims: &CoreIdTokenClaims = token_response
                    .extra_fields()
                    .id_token()
                    .expect("Server did not return an ID token")
                    .claims(&id_token_verifier, &self.nonce)
                    .map_err(|err| {
                        error!("Error is: {:?}", err);
                        SigstoreError::ClaimsVerificationError
                    })?;
                return Ok((id_token_claims.clone(), id_token.clone()));
            }
        }
        unreachable!()
    }
}

#[test]
fn test_auth_url() {
    let oidc_url = OpenIDAuthorize::new(
        "sigstore",
        "some_secret",
        "https://oauth2.sigstore.dev/auth",
        "http://localhost:8080",
    )
    .auth_url();
    let oidc_url = oidc_url.unwrap();
    assert!(oidc_url
        .0
        .to_string()
        .contains("https://oauth2.sigstore.dev/auth"));
    assert!(oidc_url.0.to_string().contains("response_type=code"));
    assert!(oidc_url.0.to_string().contains("client_id=sigstore"));
    assert!(oidc_url.0.to_string().contains("scope=openid+email"));
}
