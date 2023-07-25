// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::cmp::min;
use std::sync::Arc;

use anyhow::Result;
use lazy_static::lazy_static;
use opentelemetry::global;
use opentelemetry::runtime::Tokio;
use tracing::{span, subscriber::NoSubscriber, Span, Subscriber};
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;

const DEFAULT_JAEGER_URL: &str = "http://localhost:14268/api/traces";

lazy_static! {
    /// The ROOTSPAN is a phantom span that is running by calling [`trace_enter_root()`] at the background
    /// once the configuration is read and config.runtime.enable_tracing is enabled
    /// The ROOTSPAN exits by calling [`trace_exit_root()`] on shutdown request sent from containerd
    ///
    /// NOTE:
    ///     This allows other threads which are not directly running under some spans to be tracked easily
    ///     within the entire sandbox's lifetime.
    ///     To do this, you just need to add attribute #[instrment(parent=&(*ROOTSPAN))]
    pub static ref ROOTSPAN: Span = span!(tracing::Level::TRACE, "root-span");
}

/// The tracer wrapper for kata-containers
/// The fields and member methods should ALWAYS be PRIVATE and be exposed in a safe
/// way to other modules
unsafe impl Send for KataTracer {}
unsafe impl Sync for KataTracer {}
pub struct KataTracer {
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    enabled: bool,
}

impl Default for KataTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl KataTracer {
    /// Constructor of KataTracer, this is a dummy implementation for static initialization
    pub fn new() -> Self {
        Self {
            subscriber: Arc::new(NoSubscriber::default()),
            enabled: false,
        }
    }

    /// Set the tracing enabled flag
    fn enable(&mut self) {
        self.enabled = true;
    }

    /// Return whether the tracing is enabled, enabled by [`trace_setup`]
    fn enabled(&self) -> bool {
        self.enabled
    }

    /// Call when the tracing is enabled (set in toml configuration file)
    /// This setup the subscriber, which maintains the span's information, to global and
    /// inside KATA_TRACER.
    ///
    /// Note that the span will be noop(not collected) if a invalid subscriber is set
    pub fn trace_setup(
        &mut self,
        sid: &str,
        jaeger_endpoint: &str,
        jaeger_username: &str,
        jaeger_password: &str,
    ) -> Result<()> {
        // If varify jaeger config returns an error, it means that the tracing should not be enabled
        let endpoint = verify_jaeger_config(jaeger_endpoint, jaeger_username, jaeger_password)?;

        // derive a subscriber to collect span info
        let tracer = opentelemetry_jaeger::new_collector_pipeline()
            .with_service_name(format!("kata-sb-{}", &sid[0..min(8, sid.len())]))
            .with_endpoint(endpoint)
            .with_username(jaeger_username)
            .with_password(jaeger_password)
            .with_hyper()
            .install_batch(Tokio)?;

        let layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let sub = Registry::default().with(layer);

        // we use Arc to let global subscriber and katatracer to SHARE the SAME subscriber
        // this is for record the global subscriber into a global variable KATA_TRACER for more usages
        let subscriber = Arc::new(sub);
        tracing::subscriber::set_global_default(subscriber.clone())?;
        self.subscriber = subscriber;

        // enter the rootspan
        self.trace_enter_root();

        // modity the enable state, note that we have successfully enable tracing
        self.enable();

        info!(sl!(), "Tracing enabled successfully");
        Ok(())
    }

    /// Shutdown the tracer and emit the span info to jaeger agent
    /// The tracing information is only partially update to jaeger agent before this function is called
    pub fn trace_end(&self) {
        if self.enabled() {
            // exit the rootspan
            self.trace_exit_root();

            global::shutdown_tracer_provider();
        }
    }

    /// Enter the global ROOTSPAN
    /// This function is a hack on tracing library's guard approach, letting the span
    /// to enter without using a RAII guard to exit. This function should only be called
    /// once, and also in paired with [`trace_exit_root()`].
    fn trace_enter_root(&self) {
        self.enter_span(&ROOTSPAN);
    }

    /// Exit the global ROOTSPAN
    /// This should be called in paired with [`trace_enter_root()`].
    fn trace_exit_root(&self) {
        self.exit_span(&ROOTSPAN);
    }

    /// let the subscriber enter the span, this has to be called in pair with exit(span)
    /// This function allows **cross function span** to run without span guard
    fn enter_span(&self, span: &Span) {
        let id: Option<span::Id> = span.into();
        self.subscriber.enter(&id.unwrap());
    }

    /// let the subscriber exit the span, this has to be called in pair to enter(span)
    fn exit_span(&self, span: &Span) {
        let id: Option<span::Id> = span.into();
        self.subscriber.exit(&id.unwrap());
    }
}

/// Verifying the configuration of jaeger and setup the default value
fn verify_jaeger_config(endpoint: &str, username: &str, passwd: &str) -> Result<String> {
    if username.is_empty() && !passwd.is_empty() {
        warn!(
            sl!(),
            "Jaeger password with empty username is not allowed, tracing is NOT enabled"
        );
        return Err(anyhow::anyhow!("Empty username with non-empty password"));
    }

    // set the default endpoint address, this expects a jaeger-collector running on localhost:14268
    let endpt = if endpoint.is_empty() {
        DEFAULT_JAEGER_URL
    } else {
        endpoint
    }
    .to_owned();

    Ok(endpt)
}
