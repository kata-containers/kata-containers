// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use derivative::Derivative;
use log::{info, warn};
use tracing::subscriber::DefaultGuard;
use tracing::{span::EnteredSpan, subscriber::NoSubscriber, trace_span, Subscriber};

/// Error for dragonball tracer.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    /// Tracing is already enabled.
    #[error("Tracing is already enabled.")]
    AlreadyEnabled,
}

/// Configuration information for tracing feature.
#[derive(Clone, Derivative)]
#[derivative(Debug, PartialEq, Eq)]
pub struct TraceInfo {
    /// Sandbox id.
    pub sid: String,
    /// subscriber from runtime.
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    pub subscriber: Arc<dyn Subscriber + Send + Sync>,
}

/// drabonball tracer.
///
/// The tracer wrapper for dragonball. To enable tracing feature, you need to call
/// [`setup_tracing`] first.
pub struct DragonballTracer {
    /// Dragonball tracing subscriber.
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    /// Subscriber guard. The lifetime is as same as tracer.
    sub_guard: Option<DefaultGuard>,
    /// Tracing enabled flag.
    enabled: bool,
    /// Dragonball tracing root span.
    root_span: Option<EnteredSpan>,
}

/// The fields should always be private and the methods should be exposed in a safe.
unsafe impl Send for DragonballTracer {}

impl Default for DragonballTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl DragonballTracer {
    /// Create a dragonball tracer instance.
    pub fn new() -> Self {
        Self {
            subscriber: Arc::new(NoSubscriber::default()),
            sub_guard: None,
            enabled: false,
            root_span: None,
        }
    }

    /// Return whether the tracing is enabled, enabled by [`setup_tracing`]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// setup tracing feature.
    pub fn setup_tracing(&mut self, config: TraceInfo) -> Result<(), TraceError> {
        if self.enabled {
            warn!("Tracing is already enabled");
            return Err(TraceError::AlreadyEnabled);
        }
        self.enabled = true;
        self.subscriber = config.subscriber;

        // set the default subscriber for the current thread
        self.sub_guard = Some(tracing::subscriber::set_default(self.subscriber.clone()));
        self.root_span = Some(trace_span!("dragonball-root").entered());
        info!("Enable tracing successfully");
        Ok(())
    }

    /// end tracing for application.
    pub fn end_tracing(&mut self) -> Result<(), TraceError> {
        if self.enabled {
            // root span must drop before the default subscriber guard is dropped.
            if let Some(span) = self.root_span.take() {
                span.exit();
            }
            if let Some(guard) = self.sub_guard.take() {
                drop(guard);
            }
            self.enabled = false;
            info!("Ending tracing successfully");
        }
        Ok(())
    }
}
