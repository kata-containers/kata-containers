// Copyright (c) 2019-2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use common::message::Event;
use containerd_shim::publisher::RemotePublisher;
use containerd_shim::util::timestamp;
use containerd_shim::TtrpcContext;
use containerd_shim_protos::protobuf::well_known_types::any::Any;
use containerd_shim_protos::shim::events;
use containerd_shim_protos::shim_async::Events;
use ttrpc::MessageHeader;

// Ttrpc address passed from container runtime.
// For now containerd will pass the address, and CRI-O will not.
const TTRPC_ADDRESS_ENV: &str = "TTRPC_ADDRESS";

/// Forwarder forwards events to upper runtime.
#[async_trait]
pub(crate) trait Forwarder {
    /// Forward an event to publisher
    async fn forward(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()>;
}

/// Returns an instance of `ContainerdForwarder` in the case of
/// `TTRPC_ADDRESS` existing. Otherwise, fall back to `LogForwarder`.
pub(crate) async fn new_event_publisher(namespace: &str) -> Result<Box<dyn Forwarder>> {
    let fwd: Box<dyn Forwarder> = match env::var(TTRPC_ADDRESS_ENV) {
        Ok(address) if !address.is_empty() => Box::new(
            ContainerdForwarder::new(namespace, &address)
                .await
                .context("new containerd forwarder")?,
        ),
        // an empty address doesn't match the arm above so catch it here
        // and handle it the same way as if it's missing altogether
        Ok(_) | Err(_) => Box::new(
            LogForwarder::new(namespace)
                .await
                .context("new log forwarder")?,
        ),
    };

    Ok(fwd)
}

/// Events are forwarded to containerd via ttrpc.
struct ContainerdForwarder {
    namespace: String,
    publisher: RemotePublisher,
}

impl ContainerdForwarder {
    async fn new(namespace: &str, address: &str) -> Result<Self> {
        let publisher = RemotePublisher::new(address)
            .await
            .context("new remote publisher")?;
        Ok(Self {
            namespace: namespace.to_string(),
            publisher,
        })
    }

    fn build_forward_request(
        &self,
        event: &Arc<dyn Event + Send + Sync>,
    ) -> Result<events::ForwardRequest> {
        let mut envelope = events::Envelope::new();
        envelope.set_topic(event.r#type().clone());
        envelope.set_namespace(self.namespace.to_string());
        envelope.set_timestamp(
            timestamp().map_err(|err| anyhow!("failed to get timestamp: {:?}", err))?,
        );
        envelope.set_event(Any {
            type_url: event.type_url().clone(),
            value: event.value().context("get event value")?,
            ..Default::default()
        });

        let mut req = events::ForwardRequest::new();
        req.set_envelope(envelope);

        Ok(req)
    }
}

#[async_trait]
impl Forwarder for ContainerdForwarder {
    async fn forward(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()> {
        let req = self
            .build_forward_request(&event)
            .context("build forward request")?;
        self.publisher
            .forward(&default_ttrpc_context(), req)
            .await
            .context("forward")?;
        Ok(())
    }
}

/// Events are writen into logs.
struct LogForwarder {
    namespace: String,
}

impl LogForwarder {
    async fn new(namespace: &str) -> Result<Self> {
        Ok(Self {
            namespace: namespace.to_string(),
        })
    }
}

#[async_trait]
impl Forwarder for LogForwarder {
    async fn forward(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()> {
        let ts = timestamp().map_err(|err| anyhow!("failed to get timestamp: {:?}", err))?;
        info!(
            sl!(),
            "Received an event: topic: {}, namespace: {}, timestamp: {}, url: {}, value: {:?}",
            event.r#type(),
            self.namespace,
            ts.seconds,
            event.type_url(),
            event
        );
        Ok(())
    }
}

#[inline]
fn default_ttrpc_context() -> TtrpcContext {
    TtrpcContext {
        fd: 0,
        mh: MessageHeader::default(),
        metadata: HashMap::default(),
        timeout_nano: 0,
    }
}
