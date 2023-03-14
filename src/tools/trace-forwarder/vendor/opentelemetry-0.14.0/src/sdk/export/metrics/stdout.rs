//! Stdout Metrics Exporter
use crate::global;
use crate::sdk::{
    export::metrics::{
        CheckpointSet, Count, ExportKind, ExportKindFor, ExportKindSelector, Exporter, LastValue,
        Max, Min, Sum,
    },
    metrics::{
        aggregators::{
            ArrayAggregator, HistogramAggregator, LastValueAggregator, MinMaxSumCountAggregator,
            SumAggregator,
        },
        controllers::{self, PushController, PushControllerWorker},
        selectors::simple,
    },
};
use crate::{
    labels::{default_encoder, Encoder, LabelSet},
    metrics::{Descriptor, MetricsError, Result},
    KeyValue,
};
use futures::Stream;
#[cfg(feature = "serialize")]
use serde::{Serialize, Serializer};
use std::fmt;
use std::io;
use std::iter;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Create a new stdout exporter builder with the configuration for a stdout exporter.
pub fn stdout<S, SO, I, IS, ISI>(spawn: S, interval: I) -> StdoutExporterBuilder<io::Stdout, S, I>
where
    S: Fn(PushControllerWorker) -> SO,
    I: Fn(Duration) -> IS,
    IS: Stream<Item = ISI> + Send + 'static,
{
    StdoutExporterBuilder::<io::Stdout, S, I>::builder(spawn, interval)
}

///
#[derive(Debug)]
pub struct StdoutExporter<W> {
    /// Writer is the destination. If not set, `Stdout` is used.
    writer: Mutex<W>,
    /// Will pretty print the output sent to the writer. Default is false.
    pretty_print: bool,
    /// Suppresses timestamp printing. This is useful to create deterministic test
    /// conditions.
    do_not_print_time: bool,
    /// Encodes the labels.
    label_encoder: Box<dyn Encoder + Send + Sync>,
    /// An optional user-defined function to format a given export batch.
    formatter: Option<Formatter>,
}

/// A collection of exported lines
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[derive(Default, Debug)]
pub struct ExportBatch {
    #[cfg_attr(feature = "serialize", serde(skip_serializing_if = "Option::is_none"))]
    timestamp: Option<SystemTime>,
    lines: Vec<ExportLine>,
}

#[cfg_attr(feature = "serialize", derive(Serialize))]
#[derive(Default, Debug)]
struct ExportLine {
    name: String,
    #[cfg_attr(feature = "serialize", serde(skip_serializing_if = "Option::is_none"))]
    min: Option<ExportNumeric>,
    #[cfg_attr(feature = "serialize", serde(skip_serializing_if = "Option::is_none"))]
    max: Option<ExportNumeric>,
    #[cfg_attr(feature = "serialize", serde(skip_serializing_if = "Option::is_none"))]
    sum: Option<ExportNumeric>,
    count: u64,
    #[cfg_attr(feature = "serialize", serde(skip_serializing_if = "Option::is_none"))]
    last_value: Option<ExportNumeric>,

    #[cfg_attr(feature = "serialize", serde(skip_serializing_if = "Option::is_none"))]
    timestamp: Option<SystemTime>,
}

/// A number exported as debug for serialization
pub struct ExportNumeric(Box<dyn fmt::Debug>);

impl fmt::Debug for ExportNumeric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(feature = "serialize")]
impl Serialize for ExportNumeric {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{:?}", self);
        serializer.serialize_str(&s)
    }
}

impl<W> Exporter for StdoutExporter<W>
where
    W: fmt::Debug + io::Write,
{
    fn export(&self, checkpoint_set: &mut dyn CheckpointSet) -> Result<()> {
        let mut batch = ExportBatch::default();
        if !self.do_not_print_time {
            batch.timestamp = Some(crate::time::now());
        }
        checkpoint_set.try_for_each(self, &mut |record| {
            let agg = record.aggregator().ok_or(MetricsError::NoDataCollected)?;
            let desc = record.descriptor();
            let kind = desc.number_kind();
            let encoded_resource = record.resource().encoded(self.label_encoder.as_ref());
            let encoded_inst_labels = if !desc.instrumentation_name().is_empty() {
                let inst_labels = LabelSet::from_labels(iter::once(KeyValue::new(
                    "instrumentation.name",
                    desc.instrumentation_name().to_owned(),
                )));
                inst_labels.encoded(Some(self.label_encoder.as_ref()))
            } else {
                String::new()
            };

            let mut expose = ExportLine::default();

            if let Some(array) = agg.as_any().downcast_ref::<ArrayAggregator>() {
                expose.count = array.count()?;
            }

            if let Some(last_value) = agg.as_any().downcast_ref::<LastValueAggregator>() {
                let (value, timestamp) = last_value.last_value()?;
                expose.last_value = Some(ExportNumeric(value.to_debug(kind)));

                if !self.do_not_print_time {
                    expose.timestamp = Some(timestamp);
                }
            }

            if let Some(histogram) = agg.as_any().downcast_ref::<HistogramAggregator>() {
                expose.sum = Some(ExportNumeric(histogram.sum()?.to_debug(kind)));
                expose.count = histogram.count()?;
                // TODO expose buckets
            }

            if let Some(mmsc) = agg.as_any().downcast_ref::<MinMaxSumCountAggregator>() {
                expose.min = Some(ExportNumeric(mmsc.min()?.to_debug(kind)));
                expose.max = Some(ExportNumeric(mmsc.max()?.to_debug(kind)));
                expose.sum = Some(ExportNumeric(mmsc.sum()?.to_debug(kind)));
                expose.count = mmsc.count()?;
            }

            if let Some(sum) = agg.as_any().downcast_ref::<SumAggregator>() {
                expose.sum = Some(ExportNumeric(sum.sum()?.to_debug(kind)));
            }

            let mut encoded_labels = String::new();
            let iter = record.labels().iter();
            if let (0, _) = iter.size_hint() {
                encoded_labels = record.labels().encoded(Some(self.label_encoder.as_ref()));
            }

            let mut sb = String::new();

            sb.push_str(desc.name());

            if !encoded_labels.is_empty()
                || !encoded_resource.is_empty()
                || !encoded_inst_labels.is_empty()
            {
                sb.push('{');
                sb.push_str(&encoded_resource);
                if !encoded_inst_labels.is_empty() && !encoded_resource.is_empty() {
                    sb.push(',');
                }
                sb.push_str(&encoded_inst_labels);
                if !encoded_labels.is_empty()
                    && (!encoded_inst_labels.is_empty() || !encoded_resource.is_empty())
                {
                    sb.push(',');
                }
                sb.push_str(&encoded_labels);
                sb.push('}');
            }

            expose.name = sb;

            batch.lines.push(expose);
            Ok(())
        })?;

        self.writer.lock().map_err(From::from).and_then(|mut w| {
            let formatted = match &self.formatter {
                Some(formatter) => formatter.0(batch)?,
                None => format!("{:?}\n", batch),
            };
            w.write_all(formatted.as_bytes())
                .map_err(|e| MetricsError::Other(e.to_string()))
        })
    }
}

impl<W> ExportKindFor for StdoutExporter<W>
where
    W: fmt::Debug + io::Write,
{
    fn export_kind_for(&self, descriptor: &Descriptor) -> ExportKind {
        ExportKindSelector::Stateless.export_kind_for(descriptor)
    }
}

/// A formatter for user-defined batch serialization.
pub struct Formatter(Box<dyn Fn(ExportBatch) -> Result<String> + Send + Sync>);
impl fmt::Debug for Formatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Formatter(closure)")
    }
}

/// Configuration for a given stdout exporter.
#[derive(Debug)]
pub struct StdoutExporterBuilder<W, S, I> {
    spawn: S,
    interval: I,
    writer: Mutex<W>,
    pretty_print: bool,
    do_not_print_time: bool,
    quantiles: Option<Vec<f64>>,
    label_encoder: Option<Box<dyn Encoder + Send + Sync>>,
    period: Option<Duration>,
    formatter: Option<Formatter>,
}

impl<W, S, SO, I, IS, ISI> StdoutExporterBuilder<W, S, I>
where
    W: io::Write + fmt::Debug + Send + Sync + 'static,
    S: Fn(PushControllerWorker) -> SO,
    I: Fn(Duration) -> IS,
    IS: Stream<Item = ISI> + Send + 'static,
{
    fn builder(spawn: S, interval: I) -> StdoutExporterBuilder<io::Stdout, S, I> {
        StdoutExporterBuilder {
            spawn,
            interval,
            writer: Mutex::new(io::stdout()),
            pretty_print: false,
            do_not_print_time: false,
            quantiles: None,
            label_encoder: None,
            period: None,
            formatter: None,
        }
    }
    /// Set the writer that this exporter will use.
    pub fn with_writer<W2: io::Write>(self, writer: W2) -> StdoutExporterBuilder<W2, S, I> {
        StdoutExporterBuilder {
            spawn: self.spawn,
            interval: self.interval,
            writer: Mutex::new(writer),
            pretty_print: self.pretty_print,
            do_not_print_time: self.do_not_print_time,
            quantiles: self.quantiles,
            label_encoder: self.label_encoder,
            period: self.period,
            formatter: self.formatter,
        }
    }

    /// Set if the writer should format with pretty print
    pub fn with_pretty_print(self, pretty_print: bool) -> Self {
        StdoutExporterBuilder {
            pretty_print,
            ..self
        }
    }

    /// Hide the timestamps from exported results
    pub fn with_do_not_print_time(self, do_not_print_time: bool) -> Self {
        StdoutExporterBuilder {
            do_not_print_time,
            ..self
        }
    }

    /// Set the label encoder that this exporter will use.
    pub fn with_label_encoder<E>(self, label_encoder: E) -> Self
    where
        E: Encoder + Send + Sync + 'static,
    {
        StdoutExporterBuilder {
            label_encoder: Some(Box::new(label_encoder)),
            ..self
        }
    }

    /// Set the frequency in which metrics are exported.
    pub fn with_period(self, period: Duration) -> Self {
        StdoutExporterBuilder {
            period: Some(period),
            ..self
        }
    }

    /// Set a formatter for serializing export batch data
    pub fn with_formatter<T>(self, formatter: T) -> Self
    where
        T: Fn(ExportBatch) -> Result<String> + Send + Sync + 'static,
    {
        StdoutExporterBuilder {
            formatter: Some(Formatter(Box::new(formatter))),
            ..self
        }
    }

    /// Build a new push controller, returning errors if they arise.
    pub fn init(mut self) -> PushController {
        let period = self.period.take();
        let exporter = StdoutExporter {
            writer: self.writer,
            pretty_print: self.pretty_print,
            do_not_print_time: self.do_not_print_time,
            label_encoder: self.label_encoder.unwrap_or_else(default_encoder),
            formatter: self.formatter,
        };
        let mut push_builder = controllers::push(
            simple::Selector::Exact,
            ExportKindSelector::Stateless,
            exporter,
            self.spawn,
            self.interval,
        )
        .with_stateful(true);
        if let Some(period) = period {
            push_builder = push_builder.with_period(period);
        }

        let controller = push_builder.build();
        global::set_meter_provider(controller.provider());
        controller
    }
}
