use std::borrow::Cow;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum HttpRequestError {
    /// couldn't convert a http header value to string
    #[error("couldn't convert http header value to string for header key {key}")]
    HeaderValueToStr { key: String },

    /// unexpected error occurred
    #[error("unexpected error: {reason}")]
    Unexpected { reason: String },
}

pub trait HttpRequest {
    fn get_header_concatenated_values<'a>(&'a self, header_name: &str) -> Result<Cow<'a, str>, HttpRequestError>;
    fn get_lowercased_method(&self) -> Result<Cow<'_, str>, HttpRequestError>;
    fn get_target(&self) -> Result<Cow<'_, str>, HttpRequestError>;
}

#[cfg(feature = "http_trait_impl")]
mod http_trait_impl {
    use super::*;

    impl HttpRequest for http::request::Parts {
        fn get_header_concatenated_values<'a>(&'a self, header_name: &str) -> Result<Cow<'a, str>, HttpRequestError> {
            let mut values = Vec::new();
            let all_values = self.headers.get_all(header_name);
            for value in all_values {
                let value_str = value.to_str().map_err(|_| HttpRequestError::HeaderValueToStr {
                    key: header_name.to_owned(),
                })?;
                values.push(value_str.trim());
            }
            Ok(Cow::Owned(values.join(", ")))
        }

        fn get_lowercased_method(&self) -> Result<Cow<'_, str>, HttpRequestError> {
            Ok(Cow::Owned(self.method.as_str().to_lowercase()))
        }

        fn get_target(&self) -> Result<Cow<'_, str>, HttpRequestError> {
            Ok(Cow::Borrowed(self.uri.path()))
        }
    }
    impl<T> HttpRequest for http::request::Request<T> {
        fn get_header_concatenated_values<'a>(&'a self, header_name: &str) -> Result<Cow<'a, str>, HttpRequestError> {
            let mut values = Vec::new();
            let all_values = self.headers().get_all(header_name);
            for value in all_values {
                let value_str = value.to_str().map_err(|_| HttpRequestError::HeaderValueToStr {
                    key: header_name.to_owned(),
                })?;
                values.push(value_str.trim());
            }
            Ok(Cow::Owned(values.join(", ")))
        }

        fn get_lowercased_method(&self) -> Result<Cow<'_, str>, HttpRequestError> {
            Ok(Cow::Owned(self.method().as_str().to_lowercase()))
        }

        fn get_target(&self) -> Result<Cow<'_, str>, HttpRequestError> {
            Ok(Cow::Borrowed(self.uri().path()))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use http::method::Method;
        use http::{header, request};

        #[test]
        fn http_request_parts() {
            let req = request::Builder::new()
                .method(Method::GET)
                .uri("/foo")
                .header("Host", "example.org")
                .header(header::DATE, "Tue, 07 Jun 2014 20:51:35 GMT")
                .header("X-Example", " Example header       with some whitespace.   ")
                .header("X-EmptyHeader", "")
                .header(header::CACHE_CONTROL, "max-age=60")
                .header(header::CACHE_CONTROL, "must-revalidate")
                .body(())
                .expect("couldn't build request");

            let (parts, _) = req.into_parts();

            assert_eq!(parts.get_target().expect("target"), "/foo");
            assert_eq!(parts.get_lowercased_method().expect("method"), "get");
            assert_eq!(
                parts.get_header_concatenated_values("host").expect("host"),
                "example.org"
            );
            assert_eq!(
                parts.get_header_concatenated_values("date").expect("date"),
                "Tue, 07 Jun 2014 20:51:35 GMT"
            );
            assert_eq!(
                parts.get_header_concatenated_values("x-example").expect("example"),
                "Example header       with some whitespace."
            );
            assert_eq!(
                parts.get_header_concatenated_values("X-EmptyHeader").expect("empty"),
                ""
            );
            assert_eq!(
                parts
                    .get_header_concatenated_values(header::CACHE_CONTROL.as_str())
                    .expect("cache control"),
                "max-age=60, must-revalidate"
            );
        }

        #[test]
        fn http_request_request() {
            let req = request::Builder::new()
                .method(Method::GET)
                .uri("/foo")
                .header("Host", "example.org")
                .header(header::DATE, "Tue, 07 Jun 2014 20:51:35 GMT")
                .header("X-Example", " Example header       with some whitespace.   ")
                .header("X-EmptyHeader", "")
                .header(header::CACHE_CONTROL, "max-age=60")
                .header(header::CACHE_CONTROL, "must-revalidate")
                .body(())
                .expect("couldn't build request");

            assert_eq!(req.get_target().expect("target"), "/foo");
            assert_eq!(req.get_lowercased_method().expect("method"), "get");
            assert_eq!(req.get_header_concatenated_values("host").expect("host"), "example.org");
            assert_eq!(
                req.get_header_concatenated_values("date").expect("date"),
                "Tue, 07 Jun 2014 20:51:35 GMT"
            );
            assert_eq!(
                req.get_header_concatenated_values("x-example").expect("example"),
                "Example header       with some whitespace."
            );
            assert_eq!(req.get_header_concatenated_values("X-EmptyHeader").expect("empty"), "");
            assert_eq!(
                req.get_header_concatenated_values(header::CACHE_CONTROL.as_str())
                    .expect("cache control"),
                "max-age=60, must-revalidate"
            );
        }
    }
}
