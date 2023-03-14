use tracing::{
    callsite,
    callsite::Callsite,
    field::{self, Field, Value, Visit},
    metadata::Kind,
};

use std::{collections::HashMap, fmt};

#[derive(Default, Debug, Eq, PartialEq)]
pub struct Expect {
    fields: HashMap<String, MockValue>,
    only: bool,
}

#[derive(Debug)]
pub struct MockField {
    name: String,
    value: MockValue,
}

#[derive(Debug)]
pub enum MockValue {
    F64(f64),
    I64(i64),
    U64(u64),
    Bool(bool),
    Str(String),
    Debug(String),
    Any,
}

impl Eq for MockValue {}

impl PartialEq for MockValue {
    fn eq(&self, other: &Self) -> bool {
        use MockValue::*;

        match (self, other) {
            (F64(a), F64(b)) => {
                debug_assert!(!a.is_nan());
                debug_assert!(!b.is_nan());

                a.eq(b)
            }
            (I64(a), I64(b)) => a.eq(b),
            (U64(a), U64(b)) => a.eq(b),
            (Bool(a), Bool(b)) => a.eq(b),
            (Str(a), Str(b)) => a.eq(b),
            (Debug(a), Debug(b)) => a.eq(b),
            (Any, _) => true,
            (_, Any) => true,
            _ => false,
        }
    }
}

pub fn mock<K>(name: K) -> MockField
where
    String: From<K>,
{
    MockField {
        name: name.into(),
        value: MockValue::Any,
    }
}

pub fn msg(message: impl fmt::Display) -> MockField {
    MockField {
        name: "message".to_string(),
        value: MockValue::Debug(message.to_string()),
    }
}

impl MockField {
    /// Expect a field with the given name and value.
    pub fn with_value(self, value: &dyn Value) -> Self {
        Self {
            value: MockValue::from(value),
            ..self
        }
    }

    pub fn and(self, other: MockField) -> Expect {
        Expect {
            fields: HashMap::new(),
            only: false,
        }
        .and(self)
        .and(other)
    }

    pub fn only(self) -> Expect {
        Expect {
            fields: HashMap::new(),
            only: true,
        }
        .and(self)
    }
}

impl From<MockField> for Expect {
    fn from(field: MockField) -> Self {
        Expect {
            fields: HashMap::new(),
            only: false,
        }
        .and(field)
    }
}

impl Expect {
    pub fn and(mut self, field: MockField) -> Self {
        self.fields.insert(field.name, field.value);
        self
    }

    /// Indicates that no fields other than those specified should be expected.
    pub fn only(self) -> Self {
        Self { only: true, ..self }
    }

    fn compare_or_panic(
        &mut self,
        name: &str,
        value: &dyn Value,
        ctx: &str,
        subscriber_name: &str,
    ) {
        let value = value.into();
        match self.fields.remove(name) {
            Some(MockValue::Any) => {}
            Some(expected) => assert!(
                expected == value,
                "\n[{}] expected `{}` to contain:\n\t`{}{}`\nbut got:\n\t`{}{}`",
                subscriber_name,
                ctx,
                name,
                expected,
                name,
                value
            ),
            None if self.only => panic!(
                "[{}]expected `{}` to contain only:\n\t`{}`\nbut got:\n\t`{}{}`",
                subscriber_name, ctx, self, name, value
            ),
            _ => {}
        }
    }

    pub fn checker<'a>(&'a mut self, ctx: &'a str, subscriber_name: &'a str) -> CheckVisitor<'a> {
        CheckVisitor {
            expect: self,
            ctx,
            subscriber_name,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl fmt::Display for MockValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MockValue::F64(v) => write!(f, "f64 = {:?}", v),
            MockValue::I64(v) => write!(f, "i64 = {:?}", v),
            MockValue::U64(v) => write!(f, "u64 = {:?}", v),
            MockValue::Bool(v) => write!(f, "bool = {:?}", v),
            MockValue::Str(v) => write!(f, "&str = {:?}", v),
            MockValue::Debug(v) => write!(f, "&fmt::Debug = {:?}", v),
            MockValue::Any => write!(f, "_ = _"),
        }
    }
}

pub struct CheckVisitor<'a> {
    expect: &'a mut Expect,
    ctx: &'a str,
    subscriber_name: &'a str,
}

impl<'a> Visit for CheckVisitor<'a> {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.expect
            .compare_or_panic(field.name(), &value, self.ctx, self.subscriber_name)
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.expect
            .compare_or_panic(field.name(), &value, self.ctx, self.subscriber_name)
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.expect
            .compare_or_panic(field.name(), &value, self.ctx, self.subscriber_name)
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.expect
            .compare_or_panic(field.name(), &value, self.ctx, self.subscriber_name)
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.expect
            .compare_or_panic(field.name(), &value, self.ctx, self.subscriber_name)
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.expect.compare_or_panic(
            field.name(),
            &field::debug(value),
            self.ctx,
            self.subscriber_name,
        )
    }
}

impl<'a> CheckVisitor<'a> {
    pub fn finish(self) {
        assert!(
            self.expect.fields.is_empty(),
            "[{}] {}missing {}",
            self.subscriber_name,
            self.expect,
            self.ctx
        );
    }
}

impl<'a> From<&'a dyn Value> for MockValue {
    fn from(value: &'a dyn Value) -> Self {
        struct MockValueBuilder {
            value: Option<MockValue>,
        }

        impl Visit for MockValueBuilder {
            fn record_f64(&mut self, _: &Field, value: f64) {
                self.value = Some(MockValue::F64(value));
            }

            fn record_i64(&mut self, _: &Field, value: i64) {
                self.value = Some(MockValue::I64(value));
            }

            fn record_u64(&mut self, _: &Field, value: u64) {
                self.value = Some(MockValue::U64(value));
            }

            fn record_bool(&mut self, _: &Field, value: bool) {
                self.value = Some(MockValue::Bool(value));
            }

            fn record_str(&mut self, _: &Field, value: &str) {
                self.value = Some(MockValue::Str(value.to_owned()));
            }

            fn record_debug(&mut self, _: &Field, value: &dyn fmt::Debug) {
                self.value = Some(MockValue::Debug(format!("{:?}", value)));
            }
        }

        let fake_field = callsite!(name: "fake", kind: Kind::EVENT, fields: fake_field)
            .metadata()
            .fields()
            .field("fake_field")
            .unwrap();
        let mut builder = MockValueBuilder { value: None };
        value.record(&fake_field, &mut builder);
        builder
            .value
            .expect("finish called before a value was recorded")
    }
}

impl fmt::Display for Expect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fields ")?;
        let entries = self
            .fields
            .iter()
            .map(|(k, v)| (field::display(k), field::display(v)));
        f.debug_map().entries(entries).finish()
    }
}
