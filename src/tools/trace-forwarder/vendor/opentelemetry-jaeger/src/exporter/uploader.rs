//! # Jaeger Span Uploader
#[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
use crate::exporter::collector;
use crate::exporter::{agent, jaeger};
use opentelemetry::sdk::export::trace;

/// Uploads a batch of spans to Jaeger
#[derive(Debug)]
pub(crate) enum BatchUploader {
    /// Agent sync client
    Agent(agent::AgentAsyncClientUdp),
    /// Collector sync client
    #[cfg(any(feature = "collector_client", feature = "wasm_collector_client"))]
    Collector(collector::CollectorAsyncClientHttp),
}

impl BatchUploader {
    /// Emit a jaeger batch for the given uploader
    pub(crate) async fn upload(&mut self, batch: jaeger::Batch) -> trace::ExportResult {
        match self {
            BatchUploader::Agent(client) => {
                // TODO Implement retry behaviour
                client
                    .emit_batch(batch)
                    .await
                    .map_err::<crate::Error, _>(Into::into)?;
            }
            #[cfg(feature = "collector_client")]
            BatchUploader::Collector(collector) => {
                // TODO Implement retry behaviour
                collector.submit_batch(batch).await?;
            }
            #[cfg(all(not(feature = "collector_client"), feature = "wasm_collector_client"))]
            BatchUploader::Collector(collector) => {
                // TODO Implement retry behaviour
                collector
                    .submit_batch(batch)
                    .await
                    .map_err::<crate::Error, _>(Into::into)?;
            }
        }
        Ok(())
    }
}
