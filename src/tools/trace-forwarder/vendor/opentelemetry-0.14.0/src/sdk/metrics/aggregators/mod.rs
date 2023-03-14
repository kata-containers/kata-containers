//! Metric Aggregators
use crate::metrics::{Descriptor, InstrumentKind, MetricsError, Number, NumberKind, Result};

mod array;
mod ddsketch;
mod histogram;
mod last_value;
mod min_max_sum_count;
mod sum;

pub use array::{array, ArrayAggregator};
pub use ddsketch::{ddsketch, DdSketchAggregator, DdSketchConfig};
pub use histogram::{histogram, HistogramAggregator};
pub use last_value::{last_value, LastValueAggregator};
pub use min_max_sum_count::{min_max_sum_count, MinMaxSumCountAggregator};
pub use sum::{sum, SumAggregator};

/// RangeTest is a common routine for testing for valid input values. This
/// rejects NaN values. This rejects negative values when the metric instrument
/// does not support negative values, including monotonic counter metrics and
/// absolute ValueRecorder metrics.
pub fn range_test(number: &Number, descriptor: &Descriptor) -> Result<()> {
    if descriptor.number_kind() == &NumberKind::F64 && number.is_nan() {
        return Err(MetricsError::NaNInput);
    }

    match descriptor.instrument_kind() {
        InstrumentKind::Counter | InstrumentKind::SumObserver
            if descriptor.number_kind() == &NumberKind::F64 =>
        {
            if number.is_negative(descriptor.number_kind()) {
                return Err(MetricsError::NegativeInput);
            }
        }
        _ => (),
    };
    Ok(())
}
