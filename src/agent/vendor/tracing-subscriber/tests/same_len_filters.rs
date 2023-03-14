// These tests include field filters with no targets, so they have to go in a
// separate file.
#![cfg(feature = "env-filter")]
mod support;
use self::support::*;
use tracing::{self, subscriber::with_default, Level};
use tracing_subscriber::{filter::EnvFilter, prelude::*};

#[test]
fn same_length_targets() {
    let filter: EnvFilter = "foo=trace,bar=trace".parse().expect("filter should parse");
    let (subscriber, finished) = subscriber::mock()
        .event(event::mock().at_level(Level::TRACE))
        .event(event::mock().at_level(Level::TRACE))
        .done()
        .run_with_handle();
    let subscriber = subscriber.with(filter);

    with_default(subscriber, || {
        tracing::trace!(target: "foo", "foo");
        tracing::trace!(target: "bar", "bar");
    });

    finished.assert_finished();
}

#[test]
fn same_num_fields_event() {
    let filter: EnvFilter = "[{foo}]=trace,[{bar}]=trace"
        .parse()
        .expect("filter should parse");
    let (subscriber, finished) = subscriber::mock()
        .event(
            event::mock()
                .at_level(Level::TRACE)
                .with_fields(field::mock("foo")),
        )
        .event(
            event::mock()
                .at_level(Level::TRACE)
                .with_fields(field::mock("bar")),
        )
        .done()
        .run_with_handle();
    let subscriber = subscriber.with(filter);
    with_default(subscriber, || {
        tracing::trace!(foo = 1);
        tracing::trace!(bar = 3);
    });

    finished.assert_finished();
}

#[test]
fn same_num_fields_and_name_len() {
    let filter: EnvFilter = "[foo{bar=1}]=trace,[baz{boz=1}]=trace"
        .parse()
        .expect("filter should parse");
    let (subscriber, finished) = subscriber::mock()
        .new_span(
            span::mock()
                .named("foo")
                .at_level(Level::TRACE)
                .with_field(field::mock("bar")),
        )
        .new_span(
            span::mock()
                .named("baz")
                .at_level(Level::TRACE)
                .with_field(field::mock("boz")),
        )
        .done()
        .run_with_handle();
    let subscriber = subscriber.with(filter);
    with_default(subscriber, || {
        tracing::trace_span!("foo", bar = 1);
        tracing::trace_span!("baz", boz = 1);
    });

    finished.assert_finished();
}
