#[path = "../../tracing-futures/tests/support.rs"]
// we don't use some of the test support functions, but `tracing-futures` does.
#[allow(dead_code)]
mod support;
use support::*;

use tracing::subscriber::with_default;
use tracing::Level;
use tracing_attributes::instrument;

use std::convert::TryFrom;
use std::num::TryFromIntError;

#[instrument(err)]
fn err() -> Result<u8, TryFromIntError> {
    u8::try_from(1234)
}

#[instrument(err)]
fn err_suspicious_else() -> Result<u8, TryFromIntError> {
    {}
    u8::try_from(1234)
}

#[test]
fn test() {
    let span = span::mock().named("err");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(event::mock().at_level(Level::ERROR))
        .exit(span.clone())
        .drop_span(span)
        .done()
        .run_with_handle();
    with_default(subscriber, || err().ok());
    handle.assert_finished();
}

#[instrument(err)]
async fn err_async(polls: usize) -> Result<u8, TryFromIntError> {
    let future = PollN::new_ok(polls);
    tracing::trace!(awaiting = true);
    future.await.ok();
    u8::try_from(1234)
}

#[test]
fn test_async() {
    let span = span::mock().named("err_async");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            event::mock()
                .with_fields(field::mock("awaiting").with_value(&true))
                .at_level(Level::TRACE),
        )
        .exit(span.clone())
        .enter(span.clone())
        .event(event::mock().at_level(Level::ERROR))
        .exit(span.clone())
        .drop_span(span)
        .done()
        .run_with_handle();
    with_default(subscriber, || {
        block_on_future(async { err_async(2).await }).ok();
    });
    handle.assert_finished();
}

#[instrument(err)]
fn err_mut(out: &mut u8) -> Result<(), TryFromIntError> {
    *out = u8::try_from(1234)?;
    Ok(())
}

#[test]
fn test_mut() {
    let span = span::mock().named("err_mut");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(event::mock().at_level(Level::ERROR))
        .exit(span.clone())
        .drop_span(span)
        .done()
        .run_with_handle();
    with_default(subscriber, || err_mut(&mut 0).ok());
    handle.assert_finished();
}

#[instrument(err)]
async fn err_mut_async(polls: usize, out: &mut u8) -> Result<(), TryFromIntError> {
    let future = PollN::new_ok(polls);
    tracing::trace!(awaiting = true);
    future.await.ok();
    *out = u8::try_from(1234)?;
    Ok(())
}

#[test]
fn test_mut_async() {
    let span = span::mock().named("err_mut_async");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            event::mock()
                .with_fields(field::mock("awaiting").with_value(&true))
                .at_level(Level::TRACE),
        )
        .exit(span.clone())
        .enter(span.clone())
        .event(event::mock().at_level(Level::ERROR))
        .exit(span.clone())
        .drop_span(span)
        .done()
        .run_with_handle();
    with_default(subscriber, || {
        block_on_future(async { err_mut_async(2, &mut 0).await }).ok();
    });
    handle.assert_finished();
}

#[test]
fn impl_trait_return_type() {
    // Reproduces https://github.com/tokio-rs/tracing/issues/1227

    #[instrument(err)]
    fn returns_impl_trait(x: usize) -> Result<impl Iterator<Item = usize>, String> {
        Ok(0..x)
    }

    let span = span::mock().named("returns_impl_trait");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone()
                .with_field(field::mock("x").with_value(&10usize).only()),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .done()
        .run_with_handle();

    with_default(subscriber, || {
        for _ in returns_impl_trait(10).unwrap() {
            // nop
        }
    });

    handle.assert_finished();
}
