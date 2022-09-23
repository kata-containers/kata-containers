// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::cmp::min;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use lazy_static::lazy_static;
use opentelemetry::global;
use opentelemetry::runtime::Tokio;
use tracing::{span, subscriber::NoSubscriber, Span, Subscriber};
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;

lazy_static! {
    /// The static KATATRACER is the variable to provide public functions to outside modules
    static ref KATATRACER: Arc<Mutex<KataTracer>> = Arc::new(Mutex::new(KataTracer::new()));

    /// The ROOTSPAN is a phantom span that is running by calling [`trace_enter_root()`] at the background
    /// once the configuration is read and config.runtime.enable_tracing is enabled
    /// The ROOTSPAN exits by calling [`trace_exit_root()`] on shutdown request sent from containerd
    ///
    /// NOTE:
    ///     This allows other threads which are not directly running under some spans to be tracked easily
    ///     within the entire sandbox's lifetime.
    ///    To do this, you just need to add attribute #[instrment(parent=&(*ROOTSPAN))]
    pub static ref ROOTSPAN: Span = span!(tracing::Level::TRACE, "root-span");
}

/// The tracer wrapper for kata-containers, this contains the global static variable
/// the tracing utilities might need
/// The fields and member methods should ALWAYS be PRIVATE and be exposed in a safe
/// way to other modules
unsafe impl Send for KataTracer {}
unsafe impl Sync for KataTracer {}
struct KataTracer {
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    enabled: bool,
}

impl KataTracer {
    /// Constructor of KataTracer, this is a dummy implementation for static initialization
    fn new() -> Self {
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
}

/// Call when the tracing is enabled (set in toml configuration file)
/// This setup the subscriber, which maintains the span's information, to global and
/// inside KATATRACER.
///
/// Note that the span will be noop(not collected) if a valid subscriber is set
pub fn trace_setup(
    sid: &str,
    jaeger_endpoint: &str,
    jaeger_username: &str,
    jaeger_password: &str,
) -> Result<()> {
    let mut kt = KATATRACER.lock().unwrap();

    // If varify jaeger config returns an error, it means that the tracing should not be enabled
    let endpoint = varify_jaeger_config(jaeger_endpoint, jaeger_username, jaeger_password)?;

    // enable tracing
    kt.enable();

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
    // this is for record the global subscriber into a global variable KATATRACER for more usages
    let subscriber = Arc::new(sub);
    tracing::subscriber::set_global_default(subscriber.clone())?;
    kt.subscriber = subscriber;

    info!(sl!(), "Tracing enabled successfully");
    Ok(())
}

/// Global function to shutdown the tracer and emit the span info to jaeger agent
/// The tracing information is only partially update to jaeger agent before this function is called
pub fn trace_end() {
    if KATATRACER.lock().unwrap().enabled() {
        global::shutdown_tracer_provider();
    }
}

/// Enter the global ROOTSPAN, called at once if config.runtime.entracing is set
/// This function is a hack on tracing library's guard approach, letting the span
/// to enter without using a RAII guard to exit. This function should only be called
/// once, and also in paired with [`trace_exit_root()`].
pub fn trace_enter_root() {
    enter(&ROOTSPAN);
}

/// Exit the global ROOTSPAN, called when shutdown request is sent by containerd and the
/// shim actually does decide to shutdown the sandbox. This should be called in paired with [`trace_enter_root()`].
pub fn trace_exit_root() {
    exit(&ROOTSPAN);
}

/// let the subscriber enter the span, this has to be called in pair with exit(span)
/// This function allows **cross function span** to run without span guard
fn enter(span: &Span) {
    let kt = KATATRACER.lock().unwrap();
    let id: Option<span::Id> = span.into();
    kt.subscriber.enter(&id.unwrap());
}

/// let the subscriber exit the span, this has to be called in pair to enter(span)
fn exit(span: &Span) {
    let kt = KATATRACER.lock().unwrap();
    let id: Option<span::Id> = span.into();
    kt.subscriber.exit(&id.unwrap());
}

/// Varifying the configuration of jaeger and setup the default value
fn varify_jaeger_config(endpoint: &str, username: &str, passwd: &str) -> Result<String> {
    if username.is_empty() && !passwd.is_empty() {
        warn!(
            sl!(),
            "Jaeger password with empty username is now allowed, tracing is NOT enabled"
        );
        return Err(anyhow::anyhow!(""));
    }

    // set the default endpoint address, this expects a jaeger-collector running on localhost:14268
    let endpt = if endpoint.is_empty() {
        "http://localhost:14268/api/traces"
    } else {
        endpoint
    }
    .to_owned();

    Ok(endpt)
}
