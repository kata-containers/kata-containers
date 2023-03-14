# Open ID Connect Flow for Fulcio Signing Certificates

This is an example of the fulcio OpenID connect flow.

The general idea is to return an access_token and the email via a scope.

Both values can then be made to form a POST request to fulcio for a software
signing certificate

`cargo run --example openidflow`

The implementation contains a `redirect_listener` function that will create
a local listening server to incept the ID token and scopes returned from
sigstores OIDC service. However should you prefer, you can implement your
own redirect service and simply pass along the required values:

* client: CoreClient,
* nonce: Nonce,
* pkce_verifier: PkceCodeVerifier