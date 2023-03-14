use crate::{Key, Value};
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

static ENCODER_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Encoder is a mechanism for serializing a label set into a specific string
/// representation that supports caching, to avoid repeated serialization. An
/// example could be an exporter encoding the label set into a wire
/// representation.
pub trait Encoder: fmt::Debug {
    /// Encode returns the serialized encoding of the label
    /// set using its Iterator. This result may be cached.
    fn encode(&self, labels: &mut dyn Iterator<Item = (&Key, &Value)>) -> String;

    /// A value that is unique for each class of label encoder. Label encoders
    /// allocate these using `new_encoder_id`.
    fn id(&self) -> EncoderId;
}

/// EncoderID is used to identify distinct Encoder
/// implementations, for caching encoded results.
#[derive(Debug)]
pub struct EncoderId(usize);

impl EncoderId {
    /// Check if the id is valid
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

/// Default label encoding strategy
#[derive(Debug)]
pub struct DefaultLabelEncoder;

impl Encoder for DefaultLabelEncoder {
    fn encode(&self, labels: &mut dyn Iterator<Item = (&Key, &Value)>) -> String {
        labels
            .enumerate()
            .fold(String::new(), |mut acc, (idx, (key, value))| {
                let offset = acc.len();
                if idx > 0 {
                    acc.push(',')
                }

                if write!(acc, "{}", key).is_err() {
                    acc.truncate(offset);
                    return acc;
                }

                acc.push('=');
                if write!(acc, "{}", value).is_err() {
                    acc.truncate(offset);
                    return acc;
                }

                acc
            })
    }

    fn id(&self) -> EncoderId {
        new_encoder_id()
    }
}

/// Build a new default encoder
pub fn default_encoder() -> Box<dyn Encoder + Send + Sync> {
    Box::new(DefaultLabelEncoder)
}

/// Build a new encoder id
pub fn new_encoder_id() -> EncoderId {
    let old_encoder_id = ENCODER_ID_COUNTER.fetch_add(1, Ordering::AcqRel);
    EncoderId(old_encoder_id + 1)
}
