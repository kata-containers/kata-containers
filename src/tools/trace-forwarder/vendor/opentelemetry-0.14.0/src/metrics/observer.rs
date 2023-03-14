use crate::metrics::{
    sdk_api, AsyncRunner, Descriptor, InstrumentKind, Meter, Number, NumberKind, Observation,
    Result,
};
use crate::Unit;
use std::sync::Arc;

/// An Observer callback that can report observations for multiple instruments.
#[derive(Debug)]
pub struct BatchObserver<'a> {
    meter: &'a Meter,
}

impl<'a> BatchObserver<'a> {
    pub(crate) fn new(meter: &'a Meter) -> Self {
        BatchObserver { meter }
    }

    /// Creates a new integer `SumObserverBuilder` for `u64` values with the given name.
    pub fn u64_sum_observer<T>(&self, name: T) -> SumObserverBuilder<'_, u64>
    where
        T: Into<String>,
    {
        SumObserverBuilder::new(self.meter, name.into(), None, NumberKind::U64)
    }

    /// Creates a new floating point `SumObserverBuilder` for `f64` values with the given name.
    pub fn f64_sum_observer<T>(&self, name: T) -> SumObserverBuilder<'_, f64>
    where
        T: Into<String>,
    {
        SumObserverBuilder::new(self.meter, name.into(), None, NumberKind::F64)
    }

    /// Creates a new integer `UpDownSumObserverBuilder` for `i64` values with the given name.
    pub fn i64_up_down_sum_observer<T>(&self, name: T) -> UpDownSumObserverBuilder<'_, i64>
    where
        T: Into<String>,
    {
        UpDownSumObserverBuilder::new(self.meter, name.into(), None, NumberKind::I64)
    }

    /// Creates a new floating point `UpDownSumObserverBuilder` for `f64` values with the given name.
    pub fn f64_up_down_sum_observer<T>(&self, name: T) -> UpDownSumObserverBuilder<'_, f64>
    where
        T: Into<String>,
    {
        UpDownSumObserverBuilder::new(self.meter, name.into(), None, NumberKind::F64)
    }

    /// Creates a new integer `ValueObserverBuilder` for `u64` values with the given name.
    pub fn u64_value_observer<T>(&self, name: T) -> ValueObserverBuilder<'_, u64>
    where
        T: Into<String>,
    {
        ValueObserverBuilder::new(self.meter, name.into(), None, NumberKind::U64)
    }

    /// Creates a new integer `ValueObserverBuilder` for `i64` values with the given name.
    pub fn i64_value_observer<T>(&self, name: T) -> ValueObserverBuilder<'_, i64>
    where
        T: Into<String>,
    {
        ValueObserverBuilder::new(self.meter, name.into(), None, NumberKind::I64)
    }

    /// Creates a new floating point `ValueObserverBuilder` for `f64` values with the given name.
    pub fn f64_value_observer<T>(&self, name: T) -> ValueObserverBuilder<'_, f64>
    where
        T: Into<String>,
    {
        ValueObserverBuilder::new(self.meter, name.into(), None, NumberKind::F64)
    }
}

/// A metric that captures a precomputed sum of values at a point in time.
#[derive(Debug)]
pub struct SumObserver<T> {
    instrument: Arc<dyn sdk_api::AsyncInstrumentCore>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> SumObserver<T>
where
    T: Into<Number>,
{
    /// Returns an `Observation`: a `BatchObserverCallback` argument, for an
    /// asynchronous instrument. This returns an implementation-level
    /// object for use by the SDK, users should not refer to this.
    pub fn observation(&self, value: T) -> Observation {
        Observation::new(value.into(), self.instrument.clone())
    }
}

/// Configuration options for building a `SumObserver`
#[derive(Debug)]
pub struct SumObserverBuilder<'a, T> {
    meter: &'a Meter,
    descriptor: Descriptor,
    runner: Option<AsyncRunner>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> SumObserverBuilder<'a, T> {
    pub(crate) fn new(
        meter: &'a Meter,
        name: String,
        runner: Option<AsyncRunner>,
        number_kind: NumberKind,
    ) -> Self {
        SumObserverBuilder {
            meter,
            descriptor: Descriptor::new(
                name,
                meter.instrumentation_library().name,
                meter.instrumentation_library().version,
                InstrumentKind::SumObserver,
                number_kind,
            ),
            runner,
            _marker: std::marker::PhantomData,
        }
    }

    /// Set the description of this `SumObserver`
    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.descriptor.set_description(description.into());
        self
    }

    /// Set the unit for this `SumObserver`.
    pub fn with_unit(mut self, unit: Unit) -> Self {
        self.descriptor.config.unit = Some(unit);
        self
    }

    /// Create a `SumObserver` from this configuration.
    pub fn try_init(self) -> Result<SumObserver<T>> {
        let instrument = self
            .meter
            .new_async_instrument(self.descriptor, self.runner)?;

        Ok(SumObserver {
            instrument,
            _marker: std::marker::PhantomData,
        })
    }

    /// Create a `SumObserver` from this configuration.
    ///
    /// # Panics
    ///
    /// This method panics if it cannot create an instrument with the provided
    /// config. If you want to handle results instead, use [`try_init`]
    ///
    /// [`try_init`]: SumObserverBuilder::try_init()
    pub fn init(self) -> SumObserver<T> {
        SumObserver {
            instrument: self
                .meter
                .new_async_instrument(self.descriptor, self.runner)
                .unwrap(),
            _marker: std::marker::PhantomData,
        }
    }
}

/// A metric that captures a precomputed non-monotonic sum of values at a point
/// in time.
#[derive(Debug)]
pub struct UpDownSumObserver<T> {
    instrument: Arc<dyn sdk_api::AsyncInstrumentCore>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> UpDownSumObserver<T>
where
    T: Into<Number>,
{
    /// Returns an `Observation`: a `BatchObserverCallback` argument, for an
    /// asynchronous instrument. This returns an implementation-level
    /// object for use by the SDK, users should not refer to this.
    pub fn observation(&self, value: T) -> Observation {
        Observation::new(value.into(), self.instrument.clone())
    }
}

/// Configuration options for building a `UpDownSumObserver`
#[derive(Debug)]
pub struct UpDownSumObserverBuilder<'a, T> {
    meter: &'a Meter,
    descriptor: Descriptor,
    runner: Option<AsyncRunner>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> UpDownSumObserverBuilder<'a, T> {
    pub(crate) fn new(
        meter: &'a Meter,
        name: String,
        runner: Option<AsyncRunner>,
        number_kind: NumberKind,
    ) -> Self {
        UpDownSumObserverBuilder {
            meter,
            descriptor: Descriptor::new(
                name,
                meter.instrumentation_library().name,
                meter.instrumentation_library().version,
                InstrumentKind::UpDownSumObserver,
                number_kind,
            ),
            runner,
            _marker: std::marker::PhantomData,
        }
    }

    /// Set the description of this `UpDownSumObserver`
    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.descriptor.set_description(description.into());
        self
    }

    /// Set the unit for this `UpDownSumObserver`.
    pub fn with_unit(mut self, unit: Unit) -> Self {
        self.descriptor.config.unit = Some(unit);
        self
    }

    /// Create a `UpDownSumObserver` from this configuration.
    pub fn try_init(self) -> Result<UpDownSumObserver<T>> {
        let instrument = self
            .meter
            .new_async_instrument(self.descriptor, self.runner)?;

        Ok(UpDownSumObserver {
            instrument,
            _marker: std::marker::PhantomData,
        })
    }

    /// Create a `UpDownSumObserver` from this configuration.
    ///
    /// # Panics
    ///
    /// This method panics if it cannot create an instrument with the provided
    /// config. If you want to handle results instead, use [`try_init`]
    ///
    /// [`try_init`]: UpDownSumObserverBuilder::try_init()
    pub fn init(self) -> UpDownSumObserver<T> {
        UpDownSumObserver {
            instrument: self
                .meter
                .new_async_instrument(self.descriptor, self.runner)
                .unwrap(),
            _marker: std::marker::PhantomData,
        }
    }
}

/// A metric that captures a set of values at a point in time.
#[derive(Debug)]
pub struct ValueObserver<T> {
    instrument: Arc<dyn sdk_api::AsyncInstrumentCore>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> ValueObserver<T>
where
    T: Into<Number>,
{
    /// Returns an `Observation`: a `BatchObserverCallback` argument, for an
    /// asynchronous instrument. This returns an implementation-level
    /// object for use by the SDK, users should not refer to this.
    pub fn observation(&self, value: T) -> Observation {
        Observation::new(value.into(), self.instrument.clone())
    }
}

/// Configuration options for building a `ValueObserver`
#[derive(Debug)]
pub struct ValueObserverBuilder<'a, T> {
    meter: &'a Meter,
    descriptor: Descriptor,
    runner: Option<AsyncRunner>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> ValueObserverBuilder<'a, T> {
    pub(crate) fn new(
        meter: &'a Meter,
        name: String,
        runner: Option<AsyncRunner>,
        number_kind: NumberKind,
    ) -> Self {
        ValueObserverBuilder {
            meter,
            descriptor: Descriptor::new(
                name,
                meter.instrumentation_library().name,
                meter.instrumentation_library().version,
                InstrumentKind::ValueObserver,
                number_kind,
            ),
            runner,
            _marker: std::marker::PhantomData,
        }
    }
    /// Set the description of this `ValueObserver`
    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.descriptor.set_description(description.into());
        self
    }

    /// Set the unit for this `ValueObserver`.
    pub fn with_unit(mut self, unit: Unit) -> Self {
        self.descriptor.config.unit = Some(unit);
        self
    }

    /// Create a `ValueObserver` from this configuration.
    pub fn try_init(self) -> Result<ValueObserver<T>> {
        let instrument = self
            .meter
            .new_async_instrument(self.descriptor, self.runner)?;

        Ok(ValueObserver {
            instrument,
            _marker: std::marker::PhantomData,
        })
    }

    /// Create a `ValueObserver` from this configuration.
    ///
    /// # Panics
    ///
    /// This method panics if it cannot create an instrument with the provided
    /// config. If you want to handle results instead, use [`try_init`]
    ///
    /// [`try_init`]: ValueObserverBuilder::try_init()
    pub fn init(self) -> ValueObserver<T> {
        ValueObserver {
            instrument: self
                .meter
                .new_async_instrument(self.descriptor, self.runner)
                .unwrap(),
            _marker: std::marker::PhantomData,
        }
    }
}
