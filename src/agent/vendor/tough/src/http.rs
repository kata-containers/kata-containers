//! The `http` module provides `HttpTransport` which enables `Repository` objects to be
//! loaded over HTTP
use crate::{Transport, TransportError, TransportErrorKind};
use log::{debug, error, trace};
use reqwest::blocking::{Client, ClientBuilder, Request, Response};
use reqwest::header::{self, HeaderValue, ACCEPT_RANGES};
use reqwest::{Error, Method};
use snafu::ResultExt;
use snafu::Snafu;
use std::cmp::Ordering;
use std::io::Read;
use std::time::Duration;
use url::Url;

/// A builder for [`HttpTransport`] which allows settings customization.
///
/// # Example
///
/// ```
/// # use tough::HttpTransportBuilder;
/// let http_transport = HttpTransportBuilder::new()
/// .tries(3)
/// .backoff_factor(1.5)
/// .build();
/// ```
///
/// See [`HttpTransport`] for proxy support and other behavior details.
///
#[derive(Clone, Copy, Debug)]
pub struct HttpTransportBuilder {
    timeout: Duration,
    connect_timeout: Duration,
    tries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
    backoff_factor: f32,
}

impl Default for HttpTransportBuilder {
    fn default() -> Self {
        Self {
            timeout: std::time::Duration::from_secs(30),
            connect_timeout: std::time::Duration::from_secs(10),
            /// try / 100ms / try / 150ms / try / 225ms / try
            tries: 4,
            initial_backoff: std::time::Duration::from_millis(100),
            max_backoff: std::time::Duration::from_secs(1),
            backoff_factor: 1.5,
        }
    }
}

impl HttpTransportBuilder {
    /// Create a new `HttpTransportBuilder` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a timeout for the complete fetch operation.
    #[must_use]
    pub fn timeout(mut self, value: Duration) -> Self {
        self.timeout = value;
        self
    }

    /// Set a timeout for only the connect phase.
    #[must_use]
    pub fn connect_timeout(mut self, value: Duration) -> Self {
        self.connect_timeout = value;
        self
    }

    /// Set the total number of times we will try the fetch operation (in case of retryable
    /// failures).
    #[must_use]
    pub fn tries(mut self, value: u32) -> Self {
        self.tries = value;
        self
    }

    /// Set the pause duration between the first and second try.
    #[must_use]
    pub fn initial_backoff(mut self, value: Duration) -> Self {
        self.initial_backoff = value;
        self
    }

    /// Set the maximum duration of a pause between retries.
    #[must_use]
    pub fn max_backoff(mut self, value: Duration) -> Self {
        self.max_backoff = value;
        self
    }

    /// Set the exponential backoff factor, the factor by which the pause time will increase after
    /// each try until reaching `max_backoff`.
    #[must_use]
    pub fn backoff_factor(mut self, value: f32) -> Self {
        self.backoff_factor = value;
        self
    }

    /// Construct an [`HttpTransport`] transport from this builder's settings.
    pub fn build(self) -> HttpTransport {
        HttpTransport { settings: self }
    }
}

/// A [`Transport`] over HTTP with retry logic. Use the [`HttpTransportBuilder`] to construct a
/// custom `HttpTransport`, or use `HttpTransport::default()`.
///
/// This transport returns `FileNotFound` for the following HTTP response codes:
/// - 403: Forbidden. (Some services return this code when a file does not exist.)
/// - 404: Not Found.
/// - 410: Gone.
///
/// # Proxy Support
///
/// To use the `HttpTransport` with a proxy, specify the `HTTPS_PROXY` environment variable.
/// The transport will also respect the `NO_PROXY` environment variable.
///
#[derive(Clone, Copy, Debug, Default)]
pub struct HttpTransport {
    settings: HttpTransportBuilder,
}

/// Implement the `tough` `Transport` trait for `HttpRetryTransport`
impl Transport for HttpTransport {
    /// Send a GET request to the URL. Request will be retried per the `ClientSettings`. The
    /// returned `RetryRead` will also retry as necessary per the `ClientSettings`.
    fn fetch(&self, url: Url) -> Result<Box<dyn Read + Send>, TransportError> {
        let mut r = RetryState::new(self.settings.initial_backoff);
        Ok(Box::new(
            fetch_with_retries(&mut r, &self.settings, &url)
                .map_err(|e| TransportError::from((url, e)))?,
        ))
    }
}

/// This serves as a `Read`, but carries with it the necessary information to do retries.
#[derive(Debug)]
pub struct RetryRead {
    retry_state: RetryState,
    settings: HttpTransportBuilder,
    response: Response,
    url: Url,
}

impl Read for RetryRead {
    /// Read bytes into `buf`, retrying as necessary.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // retry loop
        loop {
            let retry_err = match self.response.read(buf) {
                Ok(sz) => {
                    self.retry_state.next_byte += sz;
                    return Ok(sz);
                }
                // store the error in `retry_err` to return later if there are no more retries
                Err(err) => err,
            };
            debug!("error during read of '{}': {:?}", self.url, retry_err);

            // increment the `retry_state` and fetch a new reader if retries are not exhausted
            if self.retry_state.current_try >= self.settings.tries - 1 {
                // we are out of retries, so return the last known error.
                return Err(retry_err);
            }
            self.retry_state.increment(&self.settings);
            self.err_if_no_range_support(retry_err)?;
            // wait, then retry the request (with a range header).
            std::thread::sleep(self.retry_state.wait);
            let new_retry_read =
                fetch_with_retries(&mut self.retry_state, &self.settings, &self.url)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            // the new fetch succeeded so we need to replace our read object with the new one.
            self.response = new_retry_read.response;
        }
    }
}

impl RetryRead {
    /// Checks for the header `Accept-Ranges: bytes`
    fn supports_range(&self) -> bool {
        if let Some(ranges) = self.response.headers().get(ACCEPT_RANGES) {
            if let Ok(val) = ranges.to_str() {
                if val.contains("bytes") {
                    return true;
                }
            }
        }
        false
    }

    /// Returns an error when we have received an error during read, but our server does not support
    /// range headers. Our retry implementation considers this a fatal condition rather that trying
    /// to start over from the beginning and advancing the `Read` to the point where failure
    /// occurred.
    fn err_if_no_range_support(&self, e: std::io::Error) -> std::io::Result<()> {
        if !self.supports_range() {
            // we cannot send a byte range request to this server, so return the error
            error!(
                "an error occurred and we cannot retry because the server \
                    does not support range requests '{}': {:?}",
                self.url, e
            );
            return Err(e);
        }
        Ok(())
    }
}

/// A private struct that serves as the retry counter.
#[derive(Clone, Copy, Debug)]
struct RetryState {
    /// The current try we are on. First try is zero.
    current_try: u32,
    /// The amount that the we should sleep before the next retry.
    wait: Duration,
    /// The next byte that we should read. e.g. the last read byte + 1.
    next_byte: usize,
}

impl RetryState {
    fn new(initial_wait: Duration) -> Self {
        Self {
            current_try: 0,
            wait: initial_wait,
            next_byte: 0,
        }
    }
}

impl RetryState {
    /// Increments the count and the wait duration.
    fn increment(&mut self, settings: &HttpTransportBuilder) {
        if self.current_try > 0 {
            let new_wait = self.wait.mul_f32(settings.backoff_factor);
            match new_wait.cmp(&settings.max_backoff) {
                Ordering::Less => {
                    self.wait = new_wait;
                }
                Ordering::Greater => {
                    self.wait = settings.max_backoff;
                }
                Ordering::Equal => {}
            }
        }
        self.current_try += 1;
    }
}

/// Sends a `GET` request to the `url`. Retries the request as necessary per the `ClientSettings`.
fn fetch_with_retries(
    r: &mut RetryState,
    cs: &HttpTransportBuilder,
    url: &Url,
) -> Result<RetryRead, HttpError> {
    trace!("beginning fetch for '{}'", url);
    // create a reqwest client
    let client = ClientBuilder::new()
        .timeout(cs.timeout)
        .connect_timeout(cs.connect_timeout)
        .build()
        .context(HttpClientSnafu)?;

    // retry loop
    loop {
        // build the request
        let request = build_request(&client, r.next_byte, url)?;

        // send the GET request, then categories the outcome by converting to an HttpResult.
        let http_result: HttpResult = client.execute(request).into();

        match http_result {
            HttpResult::Ok(response) => {
                trace!("{:?} - returning from successful fetch", r);
                return Ok(RetryRead {
                    retry_state: *r,
                    settings: *cs,
                    response,
                    url: url.clone(),
                });
            }
            HttpResult::Fatal(err) => {
                trace!("{:?} - returning fatal error from fetch: {}", r, err);
                return Err(err).context(FetchFatalSnafu);
            }
            HttpResult::FileNotFound(err) => {
                trace!("{:?} - returning file not found from fetch: {}", r, err);
                return Err(err).context(FetchFileNotFoundSnafu);
            }
            HttpResult::Retryable(err) => {
                trace!("{:?} - retryable error: {}", r, err);
                if r.current_try >= cs.tries - 1 {
                    debug!("{:?} - returning failure, no more retries: {}", r, err);
                    return Err(err).context(FetchNoMoreRetriesSnafu { tries: cs.tries });
                }
            }
        }

        r.increment(cs);
        std::thread::sleep(r.wait);
    }
}

/// Much of the complexity in the `fetch_with_retries` function is in deciphering the `Result`
/// we get from `reqwest::Client::execute`. Using this enum we categorize the states of the
/// `Result` into the categories that we need to understand.
enum HttpResult {
    /// We got a response with an HTTP code that indicates success.
    Ok(reqwest::blocking::Response),
    /// We got an `Error` (other than file-not-found) which we will not retry.
    Fatal(reqwest::Error),
    /// The file could not be found (HTTP status 403 or 404).
    FileNotFound(reqwest::Error),
    /// We received an `Error`, or we received an HTTP response code that we can retry.
    Retryable(reqwest::Error),
}

/// Takes the `Result` type from `reqwest::Client::execute`, and categorizes it into an
/// `HttpResult` variant.
impl From<Result<reqwest::blocking::Response, reqwest::Error>> for HttpResult {
    fn from(result: Result<Response, Error>) -> Self {
        match result {
            Ok(response) => {
                trace!("response received");
                // checks the status code of the response for errors
                parse_response_code(response)
            }
            Err(e) if e.is_timeout() => {
                // a connection timeout occurred
                trace!("timeout error during fetch: {}", e);
                HttpResult::Retryable(e)
            }
            Err(e) if e.is_request() => {
                // an error occurred while sending the request
                trace!("error sending request during fetch: {}", e);
                HttpResult::Retryable(e)
            }
            Err(e) => {
                // the error is not from an HTTP status code or a timeout, retries will not succeed.
                // these appear to be internal, reqwest errors and are expected to be unlikely.
                trace!("internal reqwest error during fetch: {}", e);
                HttpResult::Fatal(e)
            }
        }
    }
}

/// Checks the HTTP response code and converts a non-successful response code to an error.
fn parse_response_code(response: reqwest::blocking::Response) -> HttpResult {
    match response.error_for_status() {
        Ok(ok) => {
            trace!("response is success");
            // http status code indicates success
            HttpResult::Ok(ok)
        }
        // http status is an error
        Err(err) => match err.status() {
            None => {
                // this shouldn't happen, we received this err from the err_for_status function,
                // so the error should have a status. we cannot consider this a retryable error.
                trace!("error is fatal (no status): {}", err);
                HttpResult::Fatal(err)
            }
            Some(status) if status.is_server_error() => {
                trace!("error is retryable: {}", err);
                HttpResult::Retryable(err)
            }
            Some(status) if matches!(status.as_u16(), 403 | 404 | 410) => {
                trace!("error is file not found: {}", err);
                HttpResult::FileNotFound(err)
            }
            Some(_) => {
                trace!("error is fatal (status): {}", err);
                HttpResult::Fatal(err)
            }
        },
    }
}

/// Builds a GET request. If `next_byte` is greater than zero, adds a byte range header to the request.
fn build_request(client: &Client, next_byte: usize, url: &Url) -> Result<Request, HttpError> {
    if next_byte == 0 {
        let request = client
            .request(Method::GET, url.as_str())
            .build()
            .context(RequestBuildSnafu)?;
        Ok(request)
    } else {
        let header_value_string = format!("bytes={}-", next_byte);
        let header_value =
            HeaderValue::from_str(header_value_string.as_str()).context(InvalidHeaderSnafu {
                header_value: &header_value_string,
            })?;
        let request = client
            .request(Method::GET, url.as_str())
            .header(header::RANGE, header_value)
            .build()
            .context(RequestBuildSnafu)?;
        Ok(request)
    }
}

/// The error type for the HTTP transport module.
#[derive(Debug, Snafu)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum HttpError {
    #[snafu(display("A non-retryable error occurred: {}", source))]
    FetchFatal { source: reqwest::Error },

    #[snafu(display("File not found: {}", source))]
    FetchFileNotFound { source: reqwest::Error },

    #[snafu(display("Fetch failed after {} retries: {}", tries, source))]
    FetchNoMoreRetries { tries: u32, source: reqwest::Error },

    #[snafu(display("The HTTP client could not be built: {}", source))]
    HttpClient { source: reqwest::Error },

    #[snafu(display("Invalid header value '{}': {}", header_value, source))]
    InvalidHeader {
        header_value: String,
        source: reqwest::header::InvalidHeaderValue,
    },

    #[snafu(display("Unable to create HTTP request: {}", source))]
    RequestBuild { source: reqwest::Error },
}

/// Convert a URL `Url` and an `HttpError` into a `TransportError`
impl From<(Url, HttpError)> for TransportError {
    fn from((url, e): (Url, HttpError)) -> Self {
        match e {
            HttpError::FetchFileNotFound { .. } => {
                TransportError::new_with_cause(TransportErrorKind::FileNotFound, url, e)
            }
            _ => TransportError::new_with_cause(TransportErrorKind::Other, url, e),
        }
    }
}
