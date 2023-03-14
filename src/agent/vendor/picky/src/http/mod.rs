//! Signing HTTP Messages
//!
//! This module provides an implementation of a subset of
//! [draft-cavage-http-signatures-12 RFC](https://tools.ietf.org/html/draft-cavage-http-signatures-12).
//!
//! # Example
//! ```
//! use picky::{
//!     http::http_signature::{HttpSignatureBuilder, HttpSignature},
//!     signature::SignatureAlgorithm,
//!     hash::HashAlgorithm,
//!     key::PrivateKey,
//!     pem::parse_pem,
//! };
//! use http::{request, header::{self, HeaderName}, method::Method};
//!
//! // all you need to generate a http signature
//!
//! let private_rsa_key: &str = "-----BEGIN RSA PRIVATE KEY-----\n\
//!     MIIEpgIBAAKCAQEApDx0MjvRzmxYXKfqHy0gN1znX6rSU2EnsDTbZaU1UcsMmRNx\n\
//!     L+FqDNzwNutCSQlkujzR37+bHGTxOnRvvSG3lwRvDBZepYWPum9WDqa9T5gTS/Cj\n\
//!     luq/oSsOyt/tUDO/GcNPTTfUQlgtOZ+zRo6FA0rpAQ8CrQm7XzGQ0DMoDU1SVNnu\n\
//!     tFJowlece9Y4NtAfhA+kJ5IEmcE9AgwxJY/iCyCxUEBUe7biwbUafLdtA3+3S8Bu\n\
//!     hBXAr+1BING3qS0vl08+3eaFq5q7f7VwcYOhUmH13itqSGwDznCk4oDQl+qn9DQZ\n\
//!     X9/09KtsgxuIcozxj0RwGKX8qkz4TlAGJw+oNwIDAQABAoIBAQCN9IrimH3iFBfU\n\
//!     Dnb4d4KvF6gNMpMU6pbpYOZ51vBdQEolTX65yfZmI9mlPndOtcXQi51D7lNdmYo/\n\
//!     4kBqk2giKfzpz7QDEYyHspAJnelnkKStMNPVMBZucc8ZX6+5cOCunfg/YBAhQCHm\n\
//!     +rh0Nd+WVvtKpPTFJ/JCd48Zxf3KcDZD+AsWTjPt4zte8KdcwxiD3MrFunxgeujX\n\
//!     n0U0/f7hvX/7JBQ20gu2tD9whEaS2Gn8E4WpEV8wC6Ah1pU9mZNZ0u8clW9SV0de\n\
//!     ay0mHw8y/Wx6rkEMvrecK6mWbwSQGfRq+crI9PCwA5wn/EZmpQrQs9r5MLtDKVsQ\n\
//!     r9axQrSRAoGBANB53u3mxY9ByiYGT5Ge/33+BjANXxmIPG0TWgV1D8MhwFiaiHF+\n\
//!     tiEzoz4vi23Q+GeHyoM1wxw8VurDX+vcIJbZ0dyGOM/6F0eago7ZAtvHMdUdahAO\n\
//!     X+klqG8kIysgFXSzaU2w76816iIaXiZlDZUghrnd3wmgu9jhl3HCUlltAoGBAMms\n\
//!     2uufuk26nssF+woQuy017lgLUNFCRrO9F3iwIiyY5R/q372gsx8HVzjYGKY8CF6v\n\
//!     m6JFfxogp44ZcafYOeu+iXbqoCAK4BTdbFB7/D3rX7WgidaxUlLGoXsNFIUIVubR\n\
//!     jaRA7l3tl3fkpdqUAye6zosMKp2oybQLyX5hLAWzAoGBAJKhVUIA8W1cOaFbCPYE\n\
//!     XfExDQsZLI1ZvB5/4O47srVtdMsdDeC93b4mgqfHawr3UvAGm1KEKtIeQofmmP3c\n\
//!     mvNfCvNPWIA3h84uB6wPSKpqRUt+382hPqZOfVSGl1HKxCyL0AH78+lJQ39vCk94\n\
//!     /f+om/n46tnrupPFv+4cXi1VAoGBALzSSmYxtozwHZyYjOJvp9A8nltwvMov82J1\n\
//!     uHQW9OgsftnTXoh83Tg/9zoRmYKK0otUf7L+vnIIANjamb88g35ldu8P3bwicosW\n\
//!     hUMV0qVmqsWy+Vs5yooVzzsWlA+6LyMNMECJSqRGv3pRabesvQeFr7wgOAZE8hTQ\n\
//!     tGbPNBhhAoGBAIkXxIJT0OMKSt/A7wDE9wd3dtC8mbkqr5aZTvwiuD6OvNDdXb/J\n\
//!     i03ns56mIflifVLPYVmCEXdYIzSv7HfeR4d78bAvqiMFfnQ2PF3tuoKMSvQM0m8/\n\
//!     f3VhEFFMrUTTRMX/9PR0ITQtnZlWIDfBVgXPmWqTqCGOMYsRPv70LGse\n\
//!     -----END RSA PRIVATE KEY-----";
//! let pem = parse_pem(private_rsa_key).expect("couldn't parse pem");
//! let private_key = PrivateKey::from_pem(&pem).expect("couldn't parse private key");
//!
//! let req = request::Builder::new()
//!     .method(Method::GET)
//!     .uri("/foo")
//!     .header("Host", "example.org")
//!     .header(header::DATE, "Tue, 07 Jun 2014 20:51:35 GMT")
//!     .header("X-Example", " Example header       with some whitespace.   ")
//!     .header("X-EmptyHeader", "")
//!     .header(header::CACHE_CONTROL, "max-age=60")
//!     .header(header::CACHE_CONTROL, "must-revalidate")
//!     .body(())
//!     .expect("couldn't build request");
//!
//! let (parts, _) = req.into_parts();
//!
//! // generate http signature
//!
//! let http_signature = HttpSignatureBuilder::new()
//!     .key_id("my-rsa-key")
//!     .signature_method(&private_key, SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224))
//!     // `picky::http::http_request::HttpRequest` trait is implemented for `http::request::Parts`
//!     // for `http` crate with `http_trait_impl` feature gate
//!     .generate_signing_string_using_http_request(&parts)
//!     .request_target()
//!     .created(1402170695)
//!     .http_header("host")
//!     .http_header(header::DATE.as_str())
//!     .http_header(header::CACHE_CONTROL.as_str())
//!     .http_header("x-emptyheader")
//!     .http_header("X-EXAMPLE")
//!     .build()
//!     .expect("couldn't generate http signature");
//!
//! let http_signature_str = http_signature.to_string();
//!
//! assert_eq!(
//!     http_signature_str,
//!     "Signature keyId=\"my-rsa-key\",algorithm=\"rsa-sha224\",created=1402170695,\
//!      headers=\"(request-target) (created) host date cache-control x-emptyheader x-example\",\
//!      signature=\"QwuxxMSuvCdA5a2cDOjg+1WFEEGa/gD8fWwKm7gah4IUCssrie+bA5sp9wH7Jz8TQYh/XNDRUHKc\
//!                  0oziBAIy1CsfDQWGRM+pAonfXEJufdt07v/i0OFhj5rBJfoOWPUcJ0cXzu0gs6svNhvimS3h2g30\
//!                  gsnw1+Qjgv0+5HFwqZH4i+bHzaj0r9vIZZnnk3ecg8O2uOLuG5jCszJU9SBA0ug8l/NrQPJXMhCO\
//!                  X59HkNVCkT4TPOovNZHyJQwu8IDhba0evPTCIvrzULpN4qY+ZAua2i3wGwWqFUgbm4eBJS2pwjWr\
//!                  XyRusoELK0BjJ8a0KdOegmbEViIxy/Uqu0L2yQ==\""
//! );
//!
//! // parse a http signature and verify it
//!
//! let parsed_http_signature = http_signature_str.parse::<HttpSignature>()
//!     .expect("couldn't parse http signature");
//!
//! assert_eq!(parsed_http_signature, http_signature);
//!
//! parsed_http_signature.verifier()
//!     .signature_method(&private_key.to_public_key(), SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224))
//!     .generate_signing_string_using_http_request(&parts)
//!     .now(1402170695)
//!     .verify()
//!     .expect("couldn't verify signature");
//!
//! // alternatively you can provide a pre-generated signing string
//!
//! let signing_string =
//!     "get /foo\n\
//!      (created): 1402170695\n\
//!      host: example.org\n\
//!      date: Tue, 07 Jun 2014 20:51:35 GMT\n\
//!      cache-control: max-age=60, must-revalidate\n\
//!      x-emptyheader:\n\
//!      x-example: Example header       with some whitespace.";
//!
//! let http_signature_pre_generated = HttpSignatureBuilder::new()
//!     .key_id("my-rsa-key")
//!     .signature_method(&private_key, SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224))
//!     .pre_generated_signing_string(signing_string)
//!     .build()
//!     .expect("couldn't generate http signature using pre-generated signing string");
//!
//! let http_signature_pre_generated_str = http_signature_pre_generated.to_string();
//!
//! assert_eq!(http_signature_pre_generated, http_signature);
//! assert_eq!(http_signature_pre_generated_str, http_signature_str);
//!
//! parsed_http_signature.verifier()
//!     .signature_method(&private_key.to_public_key(), SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224))
//!     .pre_generated_signing_string(signing_string)
//!     .now(1402170695)
//!     .verify()
//!     .expect("couldn't verify signature using pre-generated signing string");
//! ```

pub mod http_request;
pub mod http_signature;

pub use http_request::HttpRequest;
pub use http_signature::HttpSignature;
