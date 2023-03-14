use super::*;

#[test]
fn filters_span_scopes() {
    let (debug_layer, debug_handle) = layer::named("debug")
        .enter(span::mock().at_level(Level::DEBUG))
        .enter(span::mock().at_level(Level::INFO))
        .enter(span::mock().at_level(Level::WARN))
        .enter(span::mock().at_level(Level::ERROR))
        .event(event::msg("hello world").in_scope(vec![
            span::mock().at_level(Level::ERROR),
            span::mock().at_level(Level::WARN),
            span::mock().at_level(Level::INFO),
            span::mock().at_level(Level::DEBUG),
        ]))
        .exit(span::mock().at_level(Level::ERROR))
        .exit(span::mock().at_level(Level::WARN))
        .exit(span::mock().at_level(Level::INFO))
        .exit(span::mock().at_level(Level::DEBUG))
        .done()
        .run_with_handle();
    let (info_layer, info_handle) = layer::named("info")
        .enter(span::mock().at_level(Level::INFO))
        .enter(span::mock().at_level(Level::WARN))
        .enter(span::mock().at_level(Level::ERROR))
        .event(event::msg("hello world").in_scope(vec![
            span::mock().at_level(Level::ERROR),
            span::mock().at_level(Level::WARN),
            span::mock().at_level(Level::INFO),
        ]))
        .exit(span::mock().at_level(Level::ERROR))
        .exit(span::mock().at_level(Level::WARN))
        .exit(span::mock().at_level(Level::INFO))
        .done()
        .run_with_handle();
    let (warn_layer, warn_handle) = layer::named("warn")
        .enter(span::mock().at_level(Level::WARN))
        .enter(span::mock().at_level(Level::ERROR))
        .event(event::msg("hello world").in_scope(vec![
            span::mock().at_level(Level::ERROR),
            span::mock().at_level(Level::WARN),
        ]))
        .exit(span::mock().at_level(Level::ERROR))
        .exit(span::mock().at_level(Level::WARN))
        .done()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(debug_layer.with_filter(LevelFilter::DEBUG))
        .with(info_layer.with_filter(LevelFilter::INFO))
        .with(warn_layer.with_filter(LevelFilter::WARN))
        .set_default();

    {
        let _trace = tracing::trace_span!("my_span").entered();
        let _debug = tracing::debug_span!("my_span").entered();
        let _info = tracing::info_span!("my_span").entered();
        let _warn = tracing::warn_span!("my_span").entered();
        let _error = tracing::error_span!("my_span").entered();
        tracing::error!("hello world");
    }

    debug_handle.assert_finished();
    info_handle.assert_finished();
    warn_handle.assert_finished();
}

#[test]
fn filters_interleaved_span_scopes() {
    fn target_layer(target: &'static str) -> (ExpectLayer, subscriber::MockHandle) {
        layer::named(format!("target_{}", target))
            .enter(span::mock().with_target(target))
            .enter(span::mock().with_target(target))
            .event(event::msg("hello world").in_scope(vec![
                span::mock().with_target(target),
                span::mock().with_target(target),
            ]))
            .event(
                event::msg("hello to my target")
                    .in_scope(vec![
                        span::mock().with_target(target),
                        span::mock().with_target(target),
                    ])
                    .with_target(target),
            )
            .exit(span::mock().with_target(target))
            .exit(span::mock().with_target(target))
            .done()
            .run_with_handle()
    }

    let (a_layer, a_handle) = target_layer("a");
    let (b_layer, b_handle) = target_layer("b");
    let (all_layer, all_handle) = layer::named("all")
        .enter(span::mock().with_target("b"))
        .enter(span::mock().with_target("a"))
        .event(event::msg("hello world").in_scope(vec![
            span::mock().with_target("a"),
            span::mock().with_target("b"),
        ]))
        .exit(span::mock().with_target("a"))
        .exit(span::mock().with_target("b"))
        .done()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(all_layer.with_filter(LevelFilter::INFO))
        .with(a_layer.with_filter(filter::filter_fn(|meta| {
            let target = meta.target();
            target == "a" || target == module_path!()
        })))
        .with(b_layer.with_filter(filter::filter_fn(|meta| {
            let target = meta.target();
            target == "b" || target == module_path!()
        })))
        .set_default();

    {
        let _a1 = tracing::trace_span!(target: "a", "a/trace").entered();
        let _b1 = tracing::info_span!(target: "b", "b/info").entered();
        let _a2 = tracing::info_span!(target: "a", "a/info").entered();
        let _b2 = tracing::trace_span!(target: "b", "b/trace").entered();
        tracing::info!("hello world");
        tracing::debug!(target: "a", "hello to my target");
        tracing::debug!(target: "b", "hello to my target");
    }

    a_handle.assert_finished();
    b_handle.assert_finished();
    all_handle.assert_finished();
}
