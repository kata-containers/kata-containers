use crate::global;
use crate::metrics::registry;
use crate::sdk::{
    export::metrics::{AggregatorSelector, Checkpointer, ExportKindFor, Exporter},
    metrics::{
        self,
        processors::{self, BasicProcessor},
        Accumulator,
    },
    Resource,
};
use futures::{channel::mpsc, task, Future, Stream, StreamExt};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time;

lazy_static::lazy_static! {
    static ref DEFAULT_PUSH_PERIOD: time::Duration = time::Duration::from_secs(10);
}

/// Create a new `PushControllerBuilder`.
pub fn push<AS, ES, E, SP, SO, I, IO>(
    aggregator_selector: AS,
    export_selector: ES,
    exporter: E,
    spawn: SP,
    interval: I,
) -> PushControllerBuilder<SP, I>
where
    AS: AggregatorSelector + Send + Sync + 'static,
    ES: ExportKindFor + Send + Sync + 'static,
    E: Exporter + Send + Sync + 'static,
    SP: Fn(PushControllerWorker) -> SO,
    I: Fn(time::Duration) -> IO,
{
    PushControllerBuilder {
        aggregator_selector: Box::new(aggregator_selector),
        export_selector: Box::new(export_selector),
        exporter: Box::new(exporter),
        spawn,
        interval,
        resource: None,
        stateful: None,
        period: None,
        timeout: None,
    }
}

/// Organizes a periodic push of metric data.
#[derive(Debug)]
pub struct PushController {
    message_sender: Mutex<mpsc::Sender<PushMessage>>,
    provider: registry::RegistryMeterProvider,
}

#[derive(Debug)]
enum PushMessage {
    Tick,
    Shutdown,
}

/// The future which executes push controller work periodically. Can be run on a
/// passed in executor.
#[allow(missing_debug_implementations)]
pub struct PushControllerWorker {
    messages: Pin<Box<dyn Stream<Item = PushMessage> + Send>>,
    accumulator: Accumulator,
    processor: Arc<BasicProcessor>,
    exporter: Box<dyn Exporter + Send + Sync>,
    _timeout: time::Duration,
}

impl PushControllerWorker {
    fn on_tick(&mut self) {
        // TODO handle timeout
        if let Err(err) = self.processor.lock().and_then(|mut checkpointer| {
            checkpointer.start_collection();
            self.accumulator.0.collect(&mut checkpointer);
            checkpointer.finish_collection()?;
            self.exporter.export(checkpointer.checkpoint_set())
        }) {
            global::handle_error(err)
        }
    }
}

impl Future for PushControllerWorker {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        loop {
            match futures::ready!(self.messages.poll_next_unpin(cx)) {
                // Span batch interval time reached, export current spans.
                Some(PushMessage::Tick) => self.on_tick(),
                // Stream has terminated or processor is shutdown, return to finish execution.
                None | Some(PushMessage::Shutdown) => {
                    return task::Poll::Ready(());
                }
            }
        }
    }
}

impl Drop for PushControllerWorker {
    fn drop(&mut self) {
        // Try to push data one last time
        self.on_tick()
    }
}

impl PushController {
    /// The controller's meter provider.
    pub fn provider(&self) -> registry::RegistryMeterProvider {
        self.provider.clone()
    }
}

impl Drop for PushController {
    fn drop(&mut self) {
        if let Ok(mut sender) = self.message_sender.lock() {
            let _ = sender.try_send(PushMessage::Shutdown);
        }
    }
}

/// Configuration for building a new `PushController`.
#[derive(Debug)]
pub struct PushControllerBuilder<S, I> {
    aggregator_selector: Box<dyn AggregatorSelector + Send + Sync>,
    export_selector: Box<dyn ExportKindFor + Send + Sync>,
    exporter: Box<dyn Exporter + Send + Sync>,
    spawn: S,
    interval: I,
    resource: Option<Resource>,
    stateful: Option<bool>,
    period: Option<time::Duration>,
    timeout: Option<time::Duration>,
}

impl<S, SO, I, IS, ISI> PushControllerBuilder<S, I>
where
    S: Fn(PushControllerWorker) -> SO,
    I: Fn(time::Duration) -> IS,
    IS: Stream<Item = ISI> + Send + 'static,
{
    /// Configure the statefulness of this controller.
    pub fn with_stateful(self, stateful: bool) -> Self {
        PushControllerBuilder {
            stateful: Some(stateful),
            ..self
        }
    }

    /// Configure the period of this controller
    pub fn with_period(self, period: time::Duration) -> Self {
        PushControllerBuilder {
            period: Some(period),
            ..self
        }
    }

    /// Configure the resource used by this controller
    pub fn with_resource(self, resource: Resource) -> Self {
        PushControllerBuilder {
            resource: Some(resource),
            ..self
        }
    }

    /// Config the timeout of one request.
    pub fn with_timeout(self, duration: time::Duration) -> Self {
        PushControllerBuilder {
            timeout: Some(duration),
            ..self
        }
    }

    /// Build a new `PushController` with this configuration.
    pub fn build(self) -> PushController {
        let processor = processors::basic(self.aggregator_selector, self.export_selector, false);
        let processor = Arc::new(processor);
        let mut accumulator = metrics::accumulator(processor.clone());

        if let Some(resource) = self.resource {
            accumulator = accumulator.with_resource(resource);
        }
        let accumulator = accumulator.build();
        let provider = registry::meter_provider(Arc::new(accumulator.clone()));

        let (message_sender, message_receiver) = mpsc::channel(256);
        let ticker =
            (self.interval)(self.period.unwrap_or(*DEFAULT_PUSH_PERIOD)).map(|_| PushMessage::Tick);

        (self.spawn)(PushControllerWorker {
            messages: Box::pin(futures::stream::select(message_receiver, ticker)),
            accumulator,
            processor,
            exporter: self.exporter,
            _timeout: self.timeout.unwrap_or(*DEFAULT_PUSH_PERIOD),
        });

        PushController {
            message_sender: Mutex::new(message_sender),
            provider,
        }
    }
}
