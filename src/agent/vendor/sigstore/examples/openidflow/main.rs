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

use anyhow::Result;
use sigstore::oauth;

fn main() -> Result<(), anyhow::Error> {
    let oidc_url = oauth::openidflow::OpenIDAuthorize::new(
        "sigstore",
        "",
        "https://oauth2.sigstore.dev/auth",
        "http://localhost:8080",
    )
    .auth_url();

    match oidc_url.as_ref() {
        Ok(url) => {
            open::that(url.0.to_string())?;
            println!(
                "Open this URL in a browser if it does not automatically open for you:\n{}\n",
                url.0.to_string()
            );
        }
        Err(e) => println!("{}", e),
    }

    let oidc_url = oidc_url?;
    let result = oauth::openidflow::RedirectListener::new(
        "127.0.0.1:8080",
        oidc_url.1, // client
        oidc_url.2, // nonce
        oidc_url.3, // pkce_verifier
    )
    .redirect_listener();

    match result {
        Ok((token_response, id_token)) => {
            println!("Email {:?}", token_response.email().unwrap().to_string());
            println!(
                "Access Token:{:?}",
                token_response.access_token_hash().unwrap().to_string()
            );
            println!("id_token: {:?}", id_token.to_string());
        }
        Err(err) => {
            println!("{}", err);
        }
    }
    anyhow::Ok(())
}
