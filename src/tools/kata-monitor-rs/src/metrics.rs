// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use prometheus::{Counter, Encoder, Gauge, Histogram, HistogramOpts, Opts, Registry, TextEncoder};

const NAMESPACE: &str = "kata_monitor";

pub struct MonitorMetrics {
    registry: Registry,
    pub running_shim_count: Gauge,
    pub scrape_count: Counter,
    pub scrape_failed_count: Counter,
    pub scrape_duration_ms: Histogram,
}

impl MonitorMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let running_shim_count = Gauge::with_opts(Opts::new(
            format!("{NAMESPACE}_running_shim_count"),
            "Running shim count(running sandboxes).",
        ))
        .unwrap();

        let scrape_count = Counter::with_opts(Opts::new(
            format!("{NAMESPACE}_scrape_count"),
            "Scrape count.",
        ))
        .unwrap();

        let scrape_failed_count = Counter::with_opts(Opts::new(
            format!("{NAMESPACE}_scrape_failed_count"),
            "Failed scrape count.",
        ))
        .unwrap();

        let scrape_duration_ms = Histogram::with_opts(
            HistogramOpts::new(
                format!("{NAMESPACE}_scrape_durations_histogram_milliseconds"),
                "Time used to scrape from shims",
            )
            .buckets(prometheus::exponential_buckets(1.0, 2.0, 10).unwrap()),
        )
        .unwrap();

        registry
            .register(Box::new(running_shim_count.clone()))
            .unwrap();
        registry.register(Box::new(scrape_count.clone())).unwrap();
        registry
            .register(Box::new(scrape_failed_count.clone()))
            .unwrap();
        registry
            .register(Box::new(scrape_duration_ms.clone()))
            .unwrap();

        Self {
            registry,
            running_shim_count,
            scrape_count,
            scrape_failed_count,
            scrape_duration_ms,
        }
    }

    pub fn encode(&self) -> Result<String> {
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

#[cfg(test)]
mod tests {}
