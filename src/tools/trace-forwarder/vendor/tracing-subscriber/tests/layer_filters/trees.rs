use super::*;

#[test]
fn basic_trees() {
    let (with_target, with_target_handle) = layer::named("info_with_target")
        .event(event::mock().at_level(Level::INFO).with_target("my_target"))
        .done()
        .run_with_handle();

    let (info, info_handle) = layer::named("info")
        .event(
            event::mock()
                .at_level(Level::INFO)
                .with_target(module_path!()),
        )
        .event(event::mock().at_level(Level::INFO).with_target("my_target"))
        .done()
        .run_with_handle();

    let (all, all_handle) = layer::named("all")
        .event(
            event::mock()
                .at_level(Level::INFO)
                .with_target(module_path!()),
        )
        .event(event::mock().at_level(Level::TRACE))
        .event(event::mock().at_level(Level::INFO).with_target("my_target"))
        .event(
            event::mock()
                .at_level(Level::TRACE)
                .with_target("my_target"),
        )
        .done()
        .run_with_handle();

    let info_tree = info
        .and_then(
            with_target.with_filter(filter::filter_fn(|meta| dbg!(meta.target()) == "my_target")),
        )
        .with_filter(LevelFilter::INFO);

    let subscriber = tracing_subscriber::registry().with(info_tree).with(all);
    let _guard = dbg!(subscriber).set_default();

    tracing::info!("hello world");
    tracing::trace!("hello trace");
    tracing::info!(target: "my_target", "hi to my target");
    tracing::trace!(target: "my_target", "hi to my target at trace");

    all_handle.assert_finished();
    info_handle.assert_finished();
    with_target_handle.assert_finished();
}

#[test]
fn filter_span_scopes() {
    fn target_layer(target: &'static str) -> (ExpectLayer, subscriber::MockHandle) {
        layer::named(format!("target_{}", target))
            .enter(span::mock().with_target(target).at_level(Level::INFO))
            .event(
                event::msg("hello world")
                    .in_scope(vec![span::mock().with_target(target).at_level(Level::INFO)]),
            )
            .exit(span::mock().with_target(target).at_level(Level::INFO))
            .done()
            .run_with_handle()
    }

    let (a_layer, a_handle) = target_layer("a");
    let (b_layer, b_handle) = target_layer("b");
    let (info_layer, info_handle) = layer::named("info")
        .enter(span::mock().with_target("b").at_level(Level::INFO))
        .enter(span::mock().with_target("a").at_level(Level::INFO))
        .event(event::msg("hello world").in_scope(vec![
            span::mock().with_target("a").at_level(Level::INFO),
            span::mock().with_target("b").at_level(Level::INFO),
        ]))
        .exit(span::mock().with_target("a").at_level(Level::INFO))
        .exit(span::mock().with_target("b").at_level(Level::INFO))
        .done()
        .run_with_handle();

    let full_scope = vec![
        span::mock().with_target("b").at_level(Level::TRACE),
        span::mock().with_target("a").at_level(Level::INFO),
        span::mock().with_target("b").at_level(Level::INFO),
        span::mock().with_target("a").at_level(Level::TRACE),
    ];
    let (all_layer, all_handle) = layer::named("all")
        .enter(span::mock().with_target("a").at_level(Level::TRACE))
        .enter(span::mock().with_target("b").at_level(Level::INFO))
        .enter(span::mock().with_target("a").at_level(Level::INFO))
        .enter(span::mock().with_target("b").at_level(Level::TRACE))
        .event(event::msg("hello world").in_scope(full_scope.clone()))
        .event(
            event::msg("hello to my target")
                .with_target("a")
                .in_scope(full_scope.clone()),
        )
        .event(
            event::msg("hello to my target")
                .with_target("b")
                .in_scope(full_scope),
        )
        .exit(span::mock().with_target("b").at_level(Level::TRACE))
        .exit(span::mock().with_target("a").at_level(Level::INFO))
        .exit(span::mock().with_target("b").at_level(Level::INFO))
        .exit(span::mock().with_target("a").at_level(Level::TRACE))
        .done()
        .run_with_handle();

    let a_layer = a_layer.with_filter(filter::filter_fn(|meta| {
        let target = meta.target();
        target == "a" || target == module_path!()
    }));

    let b_layer = b_layer.with_filter(filter::filter_fn(|meta| {
        let target = meta.target();
        target == "b" || target == module_path!()
    }));

    let info_tree = info_layer
        .and_then(a_layer)
        .and_then(b_layer)
        .with_filter(LevelFilter::INFO);

    let subscriber = tracing_subscriber::registry()
        .with(info_tree)
        .with(all_layer);
    let _guard = dbg!(subscriber).set_default();

    {
        let _a1 = tracing::trace_span!(target: "a", "a/trace").entered();
        let _b1 = tracing::info_span!(target: "b", "b/info").entered();
        let _a2 = tracing::info_span!(target: "a", "a/info").entered();
        let _b2 = tracing::trace_span!(target: "b", "b/trace").entered();
        tracing::info!("hello world");
        tracing::debug!(target: "a", "hello to my target");
        tracing::debug!(target: "b", "hello to my target");
    }

    all_handle.assert_finished();
    info_handle.assert_finished();
    a_handle.assert_finished();
    b_handle.assert_finished();
}
