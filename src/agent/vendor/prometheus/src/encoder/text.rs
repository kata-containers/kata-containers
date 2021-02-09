// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::io::Write;

use crate::errors::Result;
use crate::histogram::BUCKET_LABEL;
use crate::proto::{self, MetricFamily, MetricType};

use super::{check_metric_family, Encoder};

/// The text format of metric family.
pub const TEXT_FORMAT: &str = "text/plain; version=0.0.4";

const POSITIVE_INF: &str = "+Inf";
const QUANTILE: &str = "quantile";

/// An implementation of an [`Encoder`] that converts a [`MetricFamily`] proto message
/// into text format.
#[derive(Debug, Default)]
pub struct TextEncoder;

impl TextEncoder {
    /// Create a new text encoder.
    pub fn new() -> TextEncoder {
        TextEncoder
    }
}

impl Encoder for TextEncoder {
    fn encode<W: Write>(&self, metric_families: &[MetricFamily], writer: &mut W) -> Result<()> {
        for mf in metric_families {
            // Fail-fast checks.
            check_metric_family(mf)?;

            let name = mf.get_name();
            let help = mf.get_help();
            if !help.is_empty() {
                writeln!(writer, "# HELP {} {}", name, escape_string(help, false))?;
            }

            let metric_type = mf.get_field_type();
            let lowercase_type = format!("{:?}", metric_type).to_lowercase();
            writeln!(writer, "# TYPE {} {}", name, lowercase_type)?;

            for m in mf.get_metric() {
                match metric_type {
                    MetricType::COUNTER => {
                        write_sample(name, m, "", "", m.get_counter().get_value(), writer)?;
                    }
                    MetricType::GAUGE => {
                        write_sample(name, m, "", "", m.get_gauge().get_value(), writer)?;
                    }
                    MetricType::HISTOGRAM => {
                        let h = m.get_histogram();

                        let mut inf_seen = false;
                        for b in h.get_bucket() {
                            let upper_bound = b.get_upper_bound();
                            write_sample(
                                &format!("{}_bucket", name),
                                m,
                                BUCKET_LABEL,
                                &format!("{}", upper_bound),
                                b.get_cumulative_count() as f64,
                                writer,
                            )?;
                            if upper_bound.is_sign_positive() && upper_bound.is_infinite() {
                                inf_seen = true;
                            }
                        }
                        if !inf_seen {
                            write_sample(
                                &format!("{}_bucket", name),
                                m,
                                BUCKET_LABEL,
                                POSITIVE_INF,
                                h.get_sample_count() as f64,
                                writer,
                            )?;
                        }

                        write_sample(
                            &format!("{}_sum", name),
                            m,
                            "",
                            "",
                            h.get_sample_sum(),
                            writer,
                        )?;

                        write_sample(
                            &format!("{}_count", name),
                            m,
                            "",
                            "",
                            h.get_sample_count() as f64,
                            writer,
                        )?;
                    }
                    MetricType::SUMMARY => {
                        let s = m.get_summary();

                        for q in s.get_quantile() {
                            write_sample(
                                name,
                                m,
                                QUANTILE,
                                &format!("{}", q.get_quantile()),
                                q.get_value(),
                                writer,
                            )?;
                        }

                        write_sample(
                            &format!("{}_sum", name),
                            m,
                            "",
                            "",
                            s.get_sample_sum(),
                            writer,
                        )?;

                        write_sample(
                            &format!("{}_count", name),
                            m,
                            "",
                            "",
                            s.get_sample_count() as f64,
                            writer,
                        )?;
                    }
                    MetricType::UNTYPED => {
                        unimplemented!();
                    }
                }
            }
        }

        Ok(())
    }

    fn format_type(&self) -> &str {
        TEXT_FORMAT
    }
}

/// `write_sample` writes a single sample in text format to `writer`, given the
/// metric name, the metric proto message itself, optionally an additional label
/// name and value (use empty strings if not required), and the value.
/// The function returns the number of bytes written and any error encountered.
fn write_sample(
    name: &str,
    mc: &proto::Metric,
    additional_label_name: &str,
    additional_label_value: &str,
    value: f64,
    writer: &mut dyn Write,
) -> Result<()> {
    writer.write_all(name.as_bytes())?;

    label_pairs_to_text(
        mc.get_label(),
        additional_label_name,
        additional_label_value,
        writer,
    )?;

    write!(writer, " {}", value)?;

    let timestamp = mc.get_timestamp_ms();
    if timestamp != 0 {
        write!(writer, " {}", timestamp)?;
    }

    writer.write_all(b"\n")?;

    Ok(())
}

/// `label_pairs_to_text` converts a slice of `LabelPair` proto messages plus
/// the explicitly given additional label pair into text formatted as required
/// by the text format and writes it to `writer`. An empty slice in combination
/// with an empty string `additional_label_name` results in nothing being
/// written. Otherwise, the label pairs are written, escaped as required by the
/// text format, and enclosed in '{...}'. The function returns the number of
/// bytes written and any error encountered.
fn label_pairs_to_text(
    pairs: &[proto::LabelPair],
    additional_label_name: &str,
    additional_label_value: &str,
    writer: &mut dyn Write,
) -> Result<()> {
    if pairs.is_empty() && additional_label_name.is_empty() {
        return Ok(());
    }

    let mut separator = "{";
    for lp in pairs {
        write!(
            writer,
            "{}{}=\"{}\"",
            separator,
            lp.get_name(),
            escape_string(lp.get_value(), true)
        )?;

        separator = ",";
    }

    if !additional_label_name.is_empty() {
        write!(
            writer,
            "{}{}=\"{}\"",
            separator,
            additional_label_name,
            escape_string(additional_label_value, true)
        )?;
    }

    writer.write_all(b"}")?;

    Ok(())
}

/// `escape_string` replaces `\` by `\\`, new line character by `\n`, and `"` by `\"` if
/// `include_double_quote` is true.
fn escape_string(v: &str, include_double_quote: bool) -> String {
    let mut escaped = String::with_capacity(v.len() * 2);

    for c in v.chars() {
        match c {
            '\\' | '\n' => {
                escaped.extend(c.escape_default());
            }
            '"' if include_double_quote => {
                escaped.extend(c.escape_default());
            }
            _ => {
                escaped.push(c);
            }
        }
    }

    escaped.shrink_to_fit();

    escaped
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::counter::Counter;
    use crate::gauge::Gauge;
    use crate::histogram::{Histogram, HistogramOpts};
    use crate::metrics::{Collector, Opts};

    #[test]
    fn test_escape_string() {
        assert_eq!(r"\\", escape_string("\\", false));
        assert_eq!(r"a\\", escape_string("a\\", false));
        assert_eq!(r"\n", escape_string("\n", false));
        assert_eq!(r"a\n", escape_string("a\n", false));
        assert_eq!(r"\\n", escape_string("\\n", false));

        assert_eq!(r##"\\n\""##, escape_string("\\n\"", true));
        assert_eq!(r##"\\\n\""##, escape_string("\\\n\"", true));
        assert_eq!(r##"\\\\n\""##, escape_string("\\\\n\"", true));
        assert_eq!(r##"\"\\n\""##, escape_string("\"\\n\"", true));
    }

    #[test]
    fn test_text_encoder() {
        let counter_opts = Opts::new("test_counter", "test help")
            .const_label("a", "1")
            .const_label("b", "2");
        let counter = Counter::with_opts(counter_opts).unwrap();
        counter.inc();

        let mf = counter.collect();
        let mut writer = Vec::<u8>::new();
        let encoder = TextEncoder::new();
        let txt = encoder.encode(&mf, &mut writer);
        assert!(txt.is_ok());

        let counter_ans = r##"# HELP test_counter test help
# TYPE test_counter counter
test_counter{a="1",b="2"} 1
"##;
        assert_eq!(counter_ans.as_bytes(), writer.as_slice());

        let gauge_opts = Opts::new("test_gauge", "test help")
            .const_label("a", "1")
            .const_label("b", "2");
        let gauge = Gauge::with_opts(gauge_opts).unwrap();
        gauge.inc();
        gauge.set(42.0);

        let mf = gauge.collect();
        writer.clear();
        let txt = encoder.encode(&mf, &mut writer);
        assert!(txt.is_ok());

        let gauge_ans = r##"# HELP test_gauge test help
# TYPE test_gauge gauge
test_gauge{a="1",b="2"} 42
"##;
        assert_eq!(gauge_ans.as_bytes(), writer.as_slice());
    }

    #[test]
    fn test_text_encoder_histogram() {
        let opts = HistogramOpts::new("test_histogram", "test help").const_label("a", "1");
        let histogram = Histogram::with_opts(opts).unwrap();
        histogram.observe(0.25);

        let mf = histogram.collect();
        let mut writer = Vec::<u8>::new();
        let encoder = TextEncoder::new();
        let res = encoder.encode(&mf, &mut writer);
        assert!(res.is_ok());

        let ans = r##"# HELP test_histogram test help
# TYPE test_histogram histogram
test_histogram_bucket{a="1",le="0.005"} 0
test_histogram_bucket{a="1",le="0.01"} 0
test_histogram_bucket{a="1",le="0.025"} 0
test_histogram_bucket{a="1",le="0.05"} 0
test_histogram_bucket{a="1",le="0.1"} 0
test_histogram_bucket{a="1",le="0.25"} 1
test_histogram_bucket{a="1",le="0.5"} 1
test_histogram_bucket{a="1",le="1"} 1
test_histogram_bucket{a="1",le="2.5"} 1
test_histogram_bucket{a="1",le="5"} 1
test_histogram_bucket{a="1",le="10"} 1
test_histogram_bucket{a="1",le="+Inf"} 1
test_histogram_sum{a="1"} 0.25
test_histogram_count{a="1"} 1
"##;
        assert_eq!(ans.as_bytes(), writer.as_slice());
    }

    #[test]
    fn test_text_encoder_summary() {
        use crate::proto::{Metric, Quantile, Summary};
        use std::str;

        let mut metric_family = MetricFamily::default();
        metric_family.set_name("test_summary".to_string());
        metric_family.set_help("This is a test summary statistic".to_string());
        metric_family.set_field_type(MetricType::SUMMARY);

        let mut summary = Summary::default();
        summary.set_sample_count(5.0 as u64);
        summary.set_sample_sum(15.0);

        let mut quantile1 = Quantile::default();
        quantile1.set_quantile(50.0);
        quantile1.set_value(3.0);

        let mut quantile2 = Quantile::default();
        quantile2.set_quantile(100.0);
        quantile2.set_value(5.0);

        summary.set_quantile(from_vec!(vec!(quantile1, quantile2)));

        let mut metric = Metric::default();
        metric.set_summary(summary);
        metric_family.set_metric(from_vec!(vec!(metric)));

        let mut writer = Vec::<u8>::new();
        let encoder = TextEncoder::new();
        let res = encoder.encode(&vec![metric_family], &mut writer);
        assert!(res.is_ok());

        let ans = r##"# HELP test_summary This is a test summary statistic
# TYPE test_summary summary
test_summary{quantile="50"} 3
test_summary{quantile="100"} 5
test_summary_sum 15
test_summary_count 5
"##;
        assert_eq!(ans, str::from_utf8(writer.as_slice()).unwrap());
    }
}
