//! # HTTP Jaeger Collector Client
use http::Uri;
#[cfg(feature = "collector_client")]
use opentelemetry_http::{HttpClient, ResponseExt as _};
use std::sync::atomic::AtomicUsize;

/// `CollectorAsyncClientHttp` implements an async version of the
/// `TCollectorSyncClient` interface over HTTP
#[derive(Debug)]
pub(crate) struct CollectorAsyncClientHttp {
    endpoint: Uri,
    #[cfg(feature = "collector_client")]
    client: Box<dyn HttpClient>,
    #[cfg(all(feature = "wasm_collector_client", not(feature = "collector_client")))]
    client: WasmHttpClient,
    payload_size_estimate: AtomicUsize,
}

#[cfg(feature = "wasm_collector_client")]
#[derive(Debug)]
struct WasmHttpClient {
    auth: Option<String>,
}

#[cfg(feature = "collector_client")]
mod collector_client {
    use super::*;
    use crate::exporter::thrift::jaeger;
    use opentelemetry::sdk::export::trace::ExportResult;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use thrift::protocol::TBinaryOutputProtocol;

    impl CollectorAsyncClientHttp {
        /// Create a new HTTP collector client
        pub(crate) fn new(endpoint: Uri, client: Box<dyn HttpClient>) -> Self {
            let payload_size_estimate = AtomicUsize::new(512);

            CollectorAsyncClientHttp {
                endpoint,
                client,
                payload_size_estimate,
            }
        }

        /// Submit list of Jaeger batches
        pub(crate) async fn submit_batch(&self, batch: jaeger::Batch) -> ExportResult {
            // estimate transport capacity based on last request
            let estimate = self.payload_size_estimate.load(Ordering::Relaxed);

            // Write payload to transport buffer
            let transport = Cursor::new(Vec::with_capacity(estimate));
            let mut protocol = TBinaryOutputProtocol::new(transport, true);
            batch
                .write_to_out_protocol(&mut protocol)
                .map_err(crate::Error::from)?;

            // Use current batch capacity as new estimate
            self.payload_size_estimate
                .store(protocol.transport.get_ref().len(), Ordering::Relaxed);

            // Build collector request
            let req = http::Request::builder()
                .method(http::Method::POST)
                .uri(&self.endpoint)
                .header("Content-Type", "application/vnd.apache.thrift.binary")
                .body(protocol.transport.into_inner())
                .expect("request should always be valid");

            // Send request to collector
            let _ = self.client.send(req).await?.error_for_status()?;
            Ok(())
        }
    }
}

#[cfg(all(feature = "wasm_collector_client", not(feature = "collector_client")))]
mod wasm_collector_client {
    use super::*;
    use crate::exporter::thrift::jaeger;
    use futures_util::future;
    use http::Uri;
    use js_sys::Uint8Array;
    use std::future::Future;
    use std::io::{self, Cursor};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};
    use thrift::protocol::TBinaryOutputProtocol;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestCredentials, RequestInit, RequestMode, Response};

    impl CollectorAsyncClientHttp {
        /// Create a new HTTP collector client
        pub(crate) fn new(
            endpoint: Uri,
            username: Option<String>,
            password: Option<String>,
        ) -> thrift::Result<Self> {
            let auth = if let (Some(username), Some(password)) = (username, password) {
                let mut auth = String::from("Basic ");
                base64::encode_config_buf(username, base64::STANDARD, &mut auth);
                auth.push(':');
                base64::encode_config_buf(password, base64::STANDARD, &mut auth);
                Some(auth)
            } else {
                None
            };
            let payload_size_estimate = AtomicUsize::new(512);

            Ok(Self {
                endpoint,
                client: WasmHttpClient { auth },
                payload_size_estimate,
            })
        }

        /// Submit list of Jaeger batches
        pub(crate) fn submit_batch(
            &self,
            batch: jaeger::Batch,
        ) -> impl Future<Output = thrift::Result<jaeger::BatchSubmitResponse>> + Send + 'static
        {
            self.build_request(batch)
                .map(post_request)
                .map(|fut| future::Either::Left(SubmitBatchFuture(fut)))
                .unwrap_or_else(|e| future::Either::Right(future::err(e)))
        }

        fn build_request(&self, batch: jaeger::Batch) -> thrift::Result<Request> {
            // estimate transport capacity based on last request
            let estimate = self.payload_size_estimate.load(Ordering::Relaxed);

            // Write payload to transport buffer
            let transport = Cursor::new(Vec::with_capacity(estimate));
            let mut protocol = TBinaryOutputProtocol::new(transport, true);
            batch.write_to_out_protocol(&mut protocol)?;

            // Use current batch capacity as new estimate
            self.payload_size_estimate
                .store(protocol.transport.get_ref().len(), Ordering::Relaxed);

            // Build collector request
            let mut options = RequestInit::new();
            options.method("POST");
            options.mode(RequestMode::Cors);

            let body: Uint8Array = protocol.transport.get_ref().as_slice().into();
            options.body(Some(body.as_ref()));

            if self.client.auth.is_some() {
                options.credentials(RequestCredentials::Include);
            }

            let request = Request::new_with_str_and_init(&self.endpoint.to_string(), &options)
                .map_err(jsvalue_into_ioerror)?;
            let headers = request.headers();
            headers
                .set("Content-Type", "application/vnd.apache.thrift.binary")
                .map_err(jsvalue_into_ioerror)?;
            if let Some(auth) = self.client.auth.as_ref() {
                headers
                    .set("Authorization", auth)
                    .map_err(jsvalue_into_ioerror)?;
            }

            Ok(request)
        }
    }

    async fn post_request(request: Request) -> thrift::Result<jaeger::BatchSubmitResponse> {
        // Send request to collector
        let window = web_sys::window().unwrap();
        let res_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(jsvalue_into_ioerror)?;
        let res: Response = res_value.dyn_into().unwrap();

        if !res.ok() {
            return Err(thrift::Error::from(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Expected success response, got {} ({})",
                    res.status(),
                    res.status_text()
                ),
            )));
        }

        Ok(jaeger::BatchSubmitResponse { ok: true })
    }

    /// Wrapper of web fetch API future marked as Send.
    ///
    /// At the moment, the web APIs are single threaded. Since all opentelemetry futures are
    /// required to be Send, we mark this future as Send.
    #[pin_project::pin_project]
    struct SubmitBatchFuture<F>(#[pin] F);

    unsafe impl<F> Send for SubmitBatchFuture<F> {}

    impl<F: Future> Future for SubmitBatchFuture<F> {
        type Output = F::Output;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.project().0.poll(cx)
        }
    }

    fn jsvalue_into_ioerror(value: wasm_bindgen::JsValue) -> io::Error {
        io::Error::new(
            io::ErrorKind::Other,
            js_sys::JSON::stringify(&value)
                .map(String::from)
                .unwrap_or_else(|_| "unknown error".to_string()),
        )
    }
}
