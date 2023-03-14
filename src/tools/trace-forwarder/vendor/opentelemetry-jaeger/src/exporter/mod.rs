//! # Jaeger Exporter
//!
mod agent;
#[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
mod collector;
#[allow(clippy::all, unreachable_pub, dead_code)]
#[rustfmt::skip]
mod thrift;
mod env;
pub(crate) mod transport;
mod uploader;

use self::thrift::jaeger;
use agent::AgentAsyncClientUdp;
use async_trait::async_trait;
#[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
use collector::CollectorAsyncClientHttp;

#[cfg(feature = "isahc_collector_client")]
#[allow(unused_imports)] // this is actually used to configure authentication
use isahc::prelude::Configurable;

use opentelemetry::sdk::export::ExportError;
use opentelemetry::trace::TraceError;
use opentelemetry::{
    global,
    runtime::Runtime,
    sdk,
    sdk::export::trace,
    trace::{Event, Link, SpanKind, StatusCode, TracerProvider},
    Key, KeyValue,
};
#[cfg(feature = "collector_client")]
use opentelemetry_http::HttpClient;
use std::{
    net,
    time::{Duration, SystemTime},
};
use uploader::BatchUploader;

#[cfg(all(
    any(
        feature = "reqwest_collector_client",
        feature = "reqwest_blocking_collector_client"
    ),
    not(feature = "surf_collector_client"),
    not(feature = "isahc_collector_client")
))]
use headers::authorization::Credentials;

/// Default service name if no service is configured.
const DEFAULT_SERVICE_NAME: &str = "OpenTelemetry";

/// Default agent endpoint if none is provided
const DEFAULT_AGENT_ENDPOINT: &str = "127.0.0.1:6831";

/// Instrument Library name MUST be reported in Jaeger Span tags with the following key
const INSTRUMENTATION_LIBRARY_NAME: &str = "otel.library.name";

/// Instrument Library version MUST be reported in Jaeger Span tags with the following key
const INSTRUMENTATION_LIBRARY_VERSION: &str = "otel.library.version";

/// Create a new Jaeger exporter pipeline builder.
pub fn new_pipeline() -> PipelineBuilder {
    PipelineBuilder::default()
}

/// Jaeger span exporter
#[derive(Debug)]
pub struct Exporter {
    process: jaeger::Process,
    /// Whether or not to export instrumentation information.
    export_instrumentation_lib: bool,
    uploader: uploader::BatchUploader,
}

/// Jaeger process configuration
#[derive(Debug, Default)]
pub struct Process {
    /// Jaeger service name
    pub service_name: String,
    /// Jaeger tags
    pub tags: Vec<KeyValue>,
}

#[async_trait]
impl trace::SpanExporter for Exporter {
    /// Export spans to Jaeger
    async fn export(&mut self, batch: Vec<trace::SpanData>) -> trace::ExportResult {
        let mut jaeger_spans: Vec<jaeger::Span> = Vec::with_capacity(batch.len());
        let mut process = self.process.clone();

        for (idx, span) in batch.into_iter().enumerate() {
            if idx == 0 {
                if let Some(span_process_tags) = build_process_tags(&span) {
                    if let Some(process_tags) = &mut process.tags {
                        process_tags.extend(span_process_tags);
                    } else {
                        process.tags = Some(span_process_tags.collect())
                    }
                }
            }
            jaeger_spans.push(convert_otel_span_into_jaeger_span(
                span,
                self.export_instrumentation_lib,
            ));
        }

        self.uploader
            .upload(jaeger::Batch::new(process, jaeger_spans))
            .await
    }
}

/// Jaeger exporter builder
#[derive(Debug)]
pub struct PipelineBuilder {
    agent_endpoint: Vec<net::SocketAddr>,
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    collector_endpoint: Option<Result<http::Uri, http::uri::InvalidUri>>,
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    collector_username: Option<String>,
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    collector_password: Option<String>,
    #[cfg(feature = "collector_client")]
    client: Option<Box<dyn HttpClient>>,
    export_instrument_library: bool,
    process: Process,
    max_packet_size: Option<usize>,
    config: Option<sdk::trace::Config>,
}

impl Default for PipelineBuilder {
    /// Return the default Exporter Builder.
    fn default() -> Self {
        let builder_defaults = PipelineBuilder {
            agent_endpoint: vec![DEFAULT_AGENT_ENDPOINT.parse().unwrap()],
            #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
            collector_endpoint: None,
            #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
            collector_username: None,
            #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
            collector_password: None,
            #[cfg(feature = "collector_client")]
            client: None,
            export_instrument_library: true,
            process: Process {
                service_name: DEFAULT_SERVICE_NAME.to_string(),
                tags: Vec::new(),
            },
            max_packet_size: None,
            config: None,
        };

        // Override above defaults with env vars if set
        env::assign_attrs(builder_defaults)
    }
}

impl PipelineBuilder {
    /// Assign the agent endpoint.
    pub fn with_agent_endpoint<T: net::ToSocketAddrs>(self, agent_endpoint: T) -> Self {
        PipelineBuilder {
            agent_endpoint: agent_endpoint
                .to_socket_addrs()
                .map(|addrs| addrs.collect())
                .unwrap_or_default(),

            ..self
        }
    }

    /// Config whether to export information of instrumentation library.
    pub fn with_instrumentation_library_tags(self, export: bool) -> Self {
        PipelineBuilder {
            export_instrument_library: export,
            ..self
        }
    }

    /// Assign the collector endpoint.
    ///
    /// E.g. "http://localhost:14268/api/traces"
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "collector_client", feature = "wasm_collector_client")))
    )]
    pub fn with_collector_endpoint<T>(self, collector_endpoint: T) -> Self
    where
        http::Uri: core::convert::TryFrom<T>,
        <http::Uri as core::convert::TryFrom<T>>::Error: Into<http::uri::InvalidUri>,
    {
        PipelineBuilder {
            collector_endpoint: Some(
                core::convert::TryFrom::try_from(collector_endpoint).map_err(Into::into),
            ),
            ..self
        }
    }

    /// Assign the collector username
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    #[cfg_attr(
        docsrs,
        doc(any(feature = "collector_client", feature = "wasm_collector_client"))
    )]
    pub fn with_collector_username<S: Into<String>>(self, collector_username: S) -> Self {
        PipelineBuilder {
            collector_username: Some(collector_username.into()),
            ..self
        }
    }

    /// Assign the collector password
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    #[cfg_attr(
        docsrs,
        doc(any(feature = "collector_client", feature = "wasm_collector_client"))
    )]
    pub fn with_collector_password<S: Into<String>>(self, collector_password: S) -> Self {
        PipelineBuilder {
            collector_password: Some(collector_password.into()),
            ..self
        }
    }

    /// Assign the process service name.
    pub fn with_service_name<T: Into<String>>(mut self, service_name: T) -> Self {
        self.process.service_name = service_name.into();
        self
    }

    /// Assign the process service tags.
    pub fn with_tags<T: IntoIterator<Item = KeyValue>>(mut self, tags: T) -> Self {
        self.process.tags = tags.into_iter().collect();
        self
    }

    /// Assign the max packet size in bytes. Jaeger defaults is 65000.
    pub fn with_max_packet_size(mut self, max_packet_size: usize) -> Self {
        self.max_packet_size = Some(max_packet_size);
        self
    }

    /// Assign the SDK config for the exporter pipeline.
    pub fn with_trace_config(self, config: sdk::trace::Config) -> Self {
        PipelineBuilder {
            config: Some(config),
            ..self
        }
    }

    /// Assign the http client to use
    #[cfg(feature = "collector_client")]
    pub fn with_http_client<T: HttpClient + 'static>(mut self, client: T) -> Self {
        self.client = Some(Box::new(client));
        self
    }

    /// Install a Jaeger pipeline with a simple span processor.
    pub fn install_simple(self) -> Result<sdk::trace::Tracer, TraceError> {
        let tracer_provider = self.build_simple()?;
        let tracer =
            tracer_provider.get_tracer("opentelemetry-jaeger", Some(env!("CARGO_PKG_VERSION")));
        let _ = global::set_tracer_provider(tracer_provider);
        Ok(tracer)
    }

    /// Install a Jaeger pipeline with a batch span processor using the specified runtime.
    pub fn install_batch<R: Runtime>(self, runtime: R) -> Result<sdk::trace::Tracer, TraceError> {
        let tracer_provider = self.build_batch(runtime)?;
        let tracer =
            tracer_provider.get_tracer("opentelemetry-jaeger", Some(env!("CARGO_PKG_VERSION")));
        let _ = global::set_tracer_provider(tracer_provider);
        Ok(tracer)
    }

    /// Build a configured `sdk::trace::TracerProvider` with a simple span processor.
    pub fn build_simple(mut self) -> Result<sdk::trace::TracerProvider, TraceError> {
        let config = self.config.take();
        let exporter = self.init_exporter()?;
        let mut builder = sdk::trace::TracerProvider::builder().with_simple_exporter(exporter);
        if let Some(config) = config {
            builder = builder.with_config(config)
        }

        Ok(builder.build())
    }

    /// Build a configured `sdk::trace::TracerProvider` with a batch span processor using the
    /// specified runtime.
    pub fn build_batch<R: Runtime>(
        mut self,
        runtime: R,
    ) -> Result<sdk::trace::TracerProvider, TraceError> {
        let config = self.config.take();
        let exporter = self.init_exporter()?;
        let mut builder =
            sdk::trace::TracerProvider::builder().with_batch_exporter(exporter, runtime);
        if let Some(config) = config {
            builder = builder.with_config(config)
        }

        Ok(builder.build())
    }

    /// Initialize a new exporter.
    ///
    /// This is useful if you are manually constructing a pipeline.
    pub fn init_exporter(self) -> Result<Exporter, TraceError> {
        let export_instrumentation_lib = self.export_instrument_library;
        let (process, uploader) = self.init_uploader()?;

        Ok(Exporter {
            process: process.into(),
            export_instrumentation_lib,
            uploader,
        })
    }

    #[cfg(not(any(feature = "collector_client", feature = "wasm_collector_client")))]
    fn init_uploader(self) -> Result<(Process, BatchUploader), TraceError> {
        let agent = AgentAsyncClientUdp::new(self.agent_endpoint.as_slice(), self.max_packet_size)
            .map_err::<Error, _>(Into::into)?;
        Ok((self.process, BatchUploader::Agent(agent)))
    }

    #[cfg(feature = "collector_client")]
    fn init_uploader(self) -> Result<(Process, uploader::BatchUploader), TraceError> {
        if let Some(collector_endpoint) = self
            .collector_endpoint
            .transpose()
            .map_err::<Error, _>(Into::into)?
        {
            #[cfg(all(
                not(feature = "isahc_collector_client"),
                not(feature = "surf_collector_client"),
                not(feature = "reqwest_collector_client"),
                not(feature = "reqwest_blocking_collector_client")
            ))]
            let client = self.client.ok_or(crate::Error::NoHttpClient)?;

            #[cfg(feature = "isahc_collector_client")]
            let client = self.client.unwrap_or({
                let mut builder = isahc::HttpClient::builder();
                if let (Some(username), Some(password)) =
                    (self.collector_username, self.collector_password)
                {
                    builder = builder
                        .authentication(isahc::auth::Authentication::basic())
                        .credentials(isahc::auth::Credentials::new(username, password));
                }

                Box::new(builder.build().map_err(|err| {
                    crate::Error::ThriftAgentError(::thrift::Error::from(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        err.to_string(),
                    )))
                })?)
            });

            #[cfg(all(
                not(feature = "isahc_collector_client"),
                not(feature = "surf_collector_client"),
                any(
                    feature = "reqwest_collector_client",
                    feature = "reqwest_blocking_collector_client"
                )
            ))]
            let client = self.client.unwrap_or({
                #[cfg(feature = "reqwest_collector_client")]
                let mut builder = reqwest::ClientBuilder::new();
                #[cfg(all(
                    not(feature = "reqwest_collector_client"),
                    feature = "reqwest_blocking_collector_client"
                ))]
                let mut builder = reqwest::blocking::ClientBuilder::new();
                if let (Some(username), Some(password)) =
                    (self.collector_username, self.collector_password)
                {
                    let mut map = http::HeaderMap::with_capacity(1);
                    let auth_header_val =
                        headers::Authorization::basic(username.as_str(), password.as_str());
                    map.insert(http::header::AUTHORIZATION, auth_header_val.0.encode());
                    builder = builder.default_headers(map);
                }
                let client: Box<dyn HttpClient> =
                    Box::new(builder.build().map_err::<crate::Error, _>(Into::into)?);
                client
            });

            #[cfg(all(
                not(feature = "isahc_collector_client"),
                feature = "surf_collector_client",
                not(feature = "reqwest_collector_client"),
                not(feature = "reqwest_blocking_collector_client")
            ))]
            let client = self.client.unwrap_or({
                let client = if let (Some(username), Some(password)) =
                    (self.collector_username, self.collector_password)
                {
                    let auth = surf::http::auth::BasicAuth::new(username, password);
                    surf::Client::new().with(BasicAuthMiddleware(auth))
                } else {
                    surf::Client::new()
                };

                Box::new(client)
            });

            let collector = CollectorAsyncClientHttp::new(collector_endpoint, client);
            Ok((self.process, uploader::BatchUploader::Collector(collector)))
        } else {
            let endpoint = self.agent_endpoint.as_slice();
            let agent = AgentAsyncClientUdp::new(endpoint, self.max_packet_size)
                .map_err::<Error, _>(Into::into)?;
            Ok((self.process, BatchUploader::Agent(agent)))
        }
    }

    #[cfg(all(feature = "wasm_collector_client", not(feature = "collector_client")))]
    fn init_uploader(self) -> Result<(Process, uploader::BatchUploader), TraceError> {
        if let Some(collector_endpoint) = self
            .collector_endpoint
            .transpose()
            .map_err::<Error, _>(Into::into)?
        {
            let collector = CollectorAsyncClientHttp::new(
                collector_endpoint,
                self.collector_username,
                self.collector_password,
            )
            .map_err::<Error, _>(Into::into)?;
            Ok((self.process, uploader::BatchUploader::Collector(collector)))
        } else {
            let endpoint = self.agent_endpoint.as_slice();
            let agent = AgentAsyncClientUdp::new(endpoint, self.max_packet_size)
                .map_err::<Error, _>(Into::into)?;
            Ok((self.process, BatchUploader::Agent(agent)))
        }
    }
}

#[derive(Debug)]
#[cfg(feature = "surf_collector_client")]
struct BasicAuthMiddleware(surf::http::auth::BasicAuth);

#[async_trait]
#[cfg(feature = "surf_collector_client")]
impl surf::middleware::Middleware for BasicAuthMiddleware {
    async fn handle(
        &self,
        mut req: surf::Request,
        client: surf::Client,
        next: surf::middleware::Next<'_>,
    ) -> surf::Result<surf::Response> {
        req.insert_header(self.0.name(), self.0.value());
        next.run(req, client).await
    }
}

fn links_to_references(links: sdk::trace::EvictedQueue<Link>) -> Option<Vec<jaeger::SpanRef>> {
    if !links.is_empty() {
        let refs = links
            .iter()
            .map(|link| {
                let span_context = link.span_context();
                let trace_id = span_context.trace_id().to_u128();
                let trace_id_high = (trace_id >> 64) as i64;
                let trace_id_low = trace_id as i64;

                jaeger::SpanRef::new(
                    jaeger::SpanRefType::FollowsFrom,
                    trace_id_low,
                    trace_id_high,
                    span_context.span_id().to_u64() as i64,
                )
            })
            .collect();
        Some(refs)
    } else {
        None
    }
}

/// Convert spans to jaeger thrift span for exporting.
fn convert_otel_span_into_jaeger_span(
    span: trace::SpanData,
    export_instrument_lib: bool,
) -> jaeger::Span {
    let trace_id = span.span_context.trace_id().to_u128();
    let trace_id_high = (trace_id >> 64) as i64;
    let trace_id_low = trace_id as i64;
    jaeger::Span {
        trace_id_low,
        trace_id_high,
        span_id: span.span_context.span_id().to_u64() as i64,
        parent_span_id: span.parent_span_id.to_u64() as i64,
        operation_name: span.name.into_owned(),
        references: links_to_references(span.links),
        flags: span.span_context.trace_flags() as i32,
        start_time: span
            .start_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_micros() as i64,
        duration: span
            .end_time
            .duration_since(span.start_time)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_micros() as i64,
        tags: Some(build_span_tags(
            span.attributes,
            if export_instrument_lib {
                Some(span.instrumentation_lib)
            } else {
                None
            },
            span.status_code,
            span.status_message.into_owned(),
            span.span_kind,
        )),
        logs: events_to_logs(span.events),
    }
}

fn build_process_tags(
    span_data: &trace::SpanData,
) -> Option<impl Iterator<Item = jaeger::Tag> + '_> {
    span_data
        .resource
        .as_ref()
        .filter(|resource| !resource.is_empty())
        .map(|resource| {
            resource
                .iter()
                .map(|(k, v)| KeyValue::new(k.clone(), v.clone()).into())
        })
}

fn build_span_tags(
    attrs: sdk::trace::EvictedHashMap,
    instrumentation_lib: Option<sdk::InstrumentationLibrary>,
    status_code: StatusCode,
    status_description: String,
    kind: SpanKind,
) -> Vec<jaeger::Tag> {
    let mut user_overrides = UserOverrides::default();
    // TODO determine if namespacing is required to avoid collisions with set attributes
    let mut tags = attrs
        .into_iter()
        .map(|(k, v)| {
            user_overrides.record_attr(k.as_str());
            KeyValue::new(k, v).into()
        })
        .collect::<Vec<_>>();

    if let Some(instrumentation_lib) = instrumentation_lib {
        // Set instrument library tags
        tags.push(KeyValue::new(INSTRUMENTATION_LIBRARY_NAME, instrumentation_lib.name).into());
        if let Some(version) = instrumentation_lib.version {
            tags.push(KeyValue::new(INSTRUMENTATION_LIBRARY_VERSION, version).into())
        }
    }

    if !user_overrides.span_kind && kind != SpanKind::Internal {
        tags.push(Key::new(SPAN_KIND).string(kind.to_string()).into());
    }

    if status_code != StatusCode::Unset {
        // Ensure error status is set unless user has already overrided it
        if status_code == StatusCode::Error && !user_overrides.error {
            tags.push(Key::new(ERROR).bool(true).into());
        }
        if !user_overrides.status_code {
            tags.push(
                Key::new(OTEL_STATUS_CODE)
                    .string::<&'static str>(status_code.as_str())
                    .into(),
            );
        }
        // set status message if there is one
        if !status_description.is_empty() && !user_overrides.status_description {
            tags.push(
                Key::new(OTEL_STATUS_DESCRIPTION)
                    .string(status_description)
                    .into(),
            );
        }
    }

    tags
}

const ERROR: &str = "error";
const SPAN_KIND: &str = "span.kind";
const OTEL_STATUS_CODE: &str = "otel.status_code";
const OTEL_STATUS_DESCRIPTION: &str = "otel.status_description";

#[derive(Default)]
struct UserOverrides {
    error: bool,
    span_kind: bool,
    status_code: bool,
    status_description: bool,
}

impl UserOverrides {
    fn record_attr(&mut self, attr: &str) {
        match attr {
            ERROR => self.error = true,
            SPAN_KIND => self.span_kind = true,
            OTEL_STATUS_CODE => self.status_code = true,
            OTEL_STATUS_DESCRIPTION => self.status_description = true,
            _ => (),
        }
    }
}

fn events_to_logs(events: sdk::trace::EvictedQueue<Event>) -> Option<Vec<jaeger::Log>> {
    if events.is_empty() {
        None
    } else {
        Some(events.into_iter().map(Into::into).collect())
    }
}

/// Wrap type for errors from opentelemetry jaeger
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error from thrift agents.
    #[error("thrift agent failed with {0}")]
    ThriftAgentError(#[from] ::thrift::Error),
    /// No http client provided.
    #[cfg(feature = "collector_client")]
    #[error(
        "No http client provided. Consider enable one of the `surf_collector_client`, \
        `reqwest_collector_client`, `reqwest_blocking_collector_client`, `isahc_collector_client` \
        feature to have a default implementation. Or use with_http_client method in pipeline to \
        provide your own implementation."
    )]
    NoHttpClient,
    /// reqwest client errors
    #[error("reqwest failed with {0}")]
    #[cfg(any(
        feature = "reqwest_collector_client",
        feature = "reqwest_blocking_collector_client"
    ))]
    ReqwestClientError(#[from] reqwest::Error),

    /// invalid collector uri is provided.
    #[error("collector uri is invalid, {0}")]
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    InvalidUri(#[from] http::uri::InvalidUri),
}

impl ExportError for Error {
    fn exporter_name(&self) -> &'static str {
        "jaeger"
    }
}

#[cfg(test)]
#[cfg(feature = "collector_client")]
mod collector_client_tests {
    use crate::exporter::thrift::jaeger::Batch;
    use crate::new_pipeline;
    use opentelemetry::trace::TraceError;

    mod test_http_client {
        use async_trait::async_trait;
        use bytes::Bytes;
        use http::{Request, Response};
        use opentelemetry_http::{HttpClient, HttpError};
        use std::fmt::Debug;

        pub(crate) struct TestHttpClient;

        impl Debug for TestHttpClient {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("test http client")
            }
        }

        #[async_trait]
        impl HttpClient for TestHttpClient {
            async fn send(&self, _request: Request<Vec<u8>>) -> Result<Response<Bytes>, HttpError> {
                Err("wrong uri set in http client".into())
            }
        }
    }

    #[test]
    fn test_bring_your_own_client() -> Result<(), TraceError> {
        let (process, mut uploader) = new_pipeline()
            .with_collector_endpoint("localhost:6831")
            .with_http_client(test_http_client::TestHttpClient)
            .init_uploader()?;
        let res = futures::executor::block_on(async {
            uploader
                .upload(Batch::new(process.into(), Vec::new()))
                .await
        });
        assert_eq!(
            format!("{:?}", res.err().unwrap()),
            "Other(\"wrong uri set in http client\")"
        );

        Ok(())
    }

    #[test]
    #[cfg(any(
        feature = "isahc_collector_client",
        feature = "surf_collector_client",
        feature = "reqwest_collector_client",
        feature = "reqwest_blocking_collector_client"
    ))]
    fn test_set_collector_endpoint() {
        let invalid_uri = new_pipeline()
            .with_collector_endpoint("127.0.0.1:14268/api/traces")
            .init_uploader();
        assert!(invalid_uri.is_err());
        assert_eq!(
            format!("{:?}", invalid_uri.err().unwrap()),
            "ExportFailed(InvalidUri(InvalidUri(InvalidFormat)))"
        );

        let valid_uri = new_pipeline()
            .with_collector_endpoint("http://127.0.0.1:14268/api/traces")
            .init_uploader();

        assert!(valid_uri.is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::SPAN_KIND;
    use crate::exporter::thrift::jaeger::Tag;
    use crate::exporter::{build_span_tags, OTEL_STATUS_CODE, OTEL_STATUS_DESCRIPTION};
    use opentelemetry::sdk::trace::EvictedHashMap;
    use opentelemetry::trace::{SpanKind, StatusCode};
    use opentelemetry::KeyValue;

    fn assert_tag_contains(tags: Vec<Tag>, key: &'static str, expect_val: &'static str) {
        assert_eq!(
            tags.into_iter()
                .filter(|tag| tag.key.as_str() == key
                    && tag.v_str.as_deref().unwrap_or("") == expect_val)
                .count(),
            1,
            "Expect a tag {} with value {} but found nothing",
            key,
            expect_val
        );
    }

    fn assert_tag_not_contains(tags: Vec<Tag>, key: &'static str) {
        assert_eq!(
            tags.into_iter()
                .filter(|tag| tag.key.as_str() == key)
                .count(),
            0,
            "Not expect tag {}, but found something",
            key
        );
    }

    fn get_error_tag_test_data() -> Vec<(
        StatusCode,
        String,
        Option<&'static str>,
        Option<&'static str>,
    )> {
        // StatusCode, error message, OTEL_STATUS_CODE tag value, OTEL_STATUS_DESCRIPTION tag value
        vec![
            (StatusCode::Error, "".into(), Some("ERROR"), None),
            (StatusCode::Unset, "".into(), None, None),
            // When status is ok, no description should be in span data. This should be ensured by Otel API
            (StatusCode::Ok, "".into(), Some("OK"), None),
            (
                StatusCode::Error,
                "have message".into(),
                Some("ERROR"),
                Some("have message"),
            ),
            (StatusCode::Unset, "have message".into(), None, None),
        ]
    }

    #[test]
    fn test_set_status() {
        for (status_code, error_msg, status_tag_val, msg_tag_val) in get_error_tag_test_data() {
            let tags = build_span_tags(
                EvictedHashMap::new(20, 20),
                None,
                status_code,
                error_msg,
                SpanKind::Client,
            );
            if let Some(val) = status_tag_val {
                assert_tag_contains(tags.clone(), OTEL_STATUS_CODE, val);
            } else {
                assert_tag_not_contains(tags.clone(), OTEL_STATUS_CODE);
            }

            if let Some(val) = msg_tag_val {
                assert_tag_contains(tags.clone(), OTEL_STATUS_DESCRIPTION, val);
            } else {
                assert_tag_not_contains(tags.clone(), OTEL_STATUS_DESCRIPTION);
            }
        }
    }

    #[test]
    fn ignores_user_set_values() {
        let mut attributes = EvictedHashMap::new(20, 20);
        let user_error = true;
        let user_kind = "server";
        let user_status_code = StatusCode::Error;
        let user_status_description = "Something bad happened";
        attributes.insert(KeyValue::new("error", user_error));
        attributes.insert(KeyValue::new(SPAN_KIND, user_kind));
        attributes.insert(KeyValue::new(OTEL_STATUS_CODE, user_status_code.as_str()));
        attributes.insert(KeyValue::new(
            OTEL_STATUS_DESCRIPTION,
            user_status_description,
        ));
        let tags = build_span_tags(
            attributes,
            None,
            user_status_code,
            user_status_description.to_string(),
            SpanKind::Client,
        );

        assert!(tags
            .iter()
            .filter(|tag| tag.key.as_str() == "error")
            .all(|tag| tag.v_bool.unwrap()));
        assert_tag_contains(tags.clone(), SPAN_KIND, user_kind);
        assert_tag_contains(tags.clone(), OTEL_STATUS_CODE, user_status_code.as_str());
        assert_tag_contains(tags, OTEL_STATUS_DESCRIPTION, user_status_description);
    }
}
