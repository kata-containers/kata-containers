use {Discard, Logger};

// Separate module to test lack of imports
mod no_imports {
    use {Discard, Logger};
    /// ensure o! macro expands without error inside a module
    #[test]
    fn test_o_macro_expansion() {
        let _ = Logger::root(Discard, o!("a" => "aa"));
    }
    /// ensure o! macro expands without error inside a module
    #[test]
    fn test_slog_o_macro_expansion() {
        let _ = Logger::root(Discard, slog_o!("a" => "aa"));
    }
}

#[cfg(feature = "std")]
mod std_only {
    use super::super::*;
    use std;

    #[test]
    fn logger_fmt_debug_sanity() {
        let root = Logger::root(Discard, o!("a" => "aa"));
        let log = root.new(o!("b" => "bb", "c" => "cc"));

        assert_eq!(format!("{:?}", log), "Logger(c, b, a)");
    }

    #[test]
    fn multichain() {
        #[derive(Clone)]
        struct CheckOwned;

        impl Drain for CheckOwned {
            type Ok = ();
            type Err = Never;
            fn log(
                &self,
                record: &Record,
                values: &OwnedKVList,
            ) -> std::result::Result<Self::Ok, Self::Err> {
                assert_eq!(
                    format!("{}", record.msg()),
                    format!("{:?}", values)
                );
                Ok(())
            }
        }

        let root = Logger::root(CheckOwned, o!("a" => "aa"));
        let log = root.new(o!("b1" => "bb", "b2" => "bb"));

        info!(log, "(b2, b1, a)");

        let log = Logger::root(log, o!("c" => "cc"));
        info!(log, "(c, b2, b1, a)");
        let log = Logger::root(log, o!("d1" => "dd", "d2" => "dd"));
        info!(log, "(d2, d1, c, b2, b1, a)");
    }
}

#[test]
fn expressions() {
    use super::{Record, Result, Serializer, KV};

    struct Foo;

    impl Foo {
        fn bar(&self) -> u32 {
            1
        }
    }

    struct X {
        foo: Foo,
    }

    let log = Logger::root(Discard, o!("version" => env!("CARGO_PKG_VERSION")));

    let foo = Foo;
    let r = X { foo: foo };

    warn!(log, "logging message");
    slog_warn!(log, "logging message");

    info!(log, #"with tag", "logging message");
    slog_info!(log, #"with tag", "logging message");

    warn!(log, "logging message"; "a" => "b");
    slog_warn!(log, "logging message"; "a" => "b");

    warn!(log, "logging message bar={}", r.foo.bar());
    slog_warn!(log, "logging message bar={}", r.foo.bar());

    warn!(
        log,
        "logging message bar={} foo={}",
        r.foo.bar(),
        r.foo.bar()
    );
    slog_warn!(
        log,
        "logging message bar={} foo={}",
        r.foo.bar(),
        r.foo.bar()
    );

    // trailing comma check
    warn!(
        log,
        "logging message bar={} foo={}",
        r.foo.bar(),
        r.foo.bar(),
    );
    slog_warn!(
        log,
        "logging message bar={} foo={}",
        r.foo.bar(),
        r.foo.bar(),
    );

    warn!(log, "logging message bar={}", r.foo.bar(); "x" => 1);
    slog_warn!(log, "logging message bar={}", r.foo.bar(); "x" => 1);

    // trailing comma check
    warn!(log, "logging message bar={}", r.foo.bar(); "x" => 1,);
    slog_warn!(log, "logging message bar={}", r.foo.bar(); "x" => 1,);

    warn!(log,
          "logging message bar={}", r.foo.bar(); "x" => 1, "y" => r.foo.bar());
    slog_warn!(log,
               "logging message bar={}", r.foo.bar();
               "x" => 1, "y" => r.foo.bar());

    warn!(log, "logging message bar={}", r.foo.bar(); "x" => r.foo.bar());
    slog_warn!(log, "logging message bar={}", r.foo.bar(); "x" => r.foo.bar());

    warn!(log, "logging message bar={}", r.foo.bar();
          "x" => r.foo.bar(), "y" => r.foo.bar());
    slog_warn!(log,
               "logging message bar={}", r.foo.bar();
               "x" => r.foo.bar(), "y" => r.foo.bar());

    // trailing comma check
    warn!(log,
          "logging message bar={}", r.foo.bar();
          "x" => r.foo.bar(), "y" => r.foo.bar(),);
    slog_warn!(log,
               "logging message bar={}", r.foo.bar();
               "x" => r.foo.bar(), "y" => r.foo.bar(),);

    {
        #[derive(Clone)]
        struct K;

        impl KV for K {
            fn serialize(
                &self,
                _record: &Record,
                _serializer: &mut Serializer,
            ) -> Result {
                Ok(())
            }
        }

        let x = K;

        let _log = log.new(o!(x.clone()));
        let _log = log.new(o!("foo" => "bar", x.clone()));
        let _log = log.new(o!("foo" => "bar", x.clone(), x.clone()));
        let _log = log.new(
            slog_o!("foo" => "bar", x.clone(), x.clone(), "aaa" => "bbb"),
        );

        info!(log, "message"; "foo" => "bar", &x, &x, "aaa" => "bbb");
    }

    info!(
        log,
        "message {}",
          { 3 + 3; 2};
          "foo" => "bar",
          "foo" => { 3 + 3; 2},
          "aaa" => "bbb");
}

#[cfg(integer128)]
#[test]
fn integer_128_types() {
    let log = Logger::root(Discard, o!("version" => env!("CARGO_PKG_VERSION")));

    info!(log, "i128 = {}", 42i128; "foo" => 7i128);
    info!(log, "u128 = {}", 42u128; "foo" => 7u128);
}

#[test]
fn expressions_fmt() {
    let log = Logger::root(Discard, o!("version" => env!("CARGO_PKG_VERSION")));

    let f = "f";
    let d = (1, 2);

    info!(log, "message"; "f" => %f, "d" => ?d);
}

#[test]
fn makers() {
    use ::*;
    let drain = Duplicate(
        Discard.filter(|r| r.level().is_at_least(Level::Info)),
        Discard.filter_level(Level::Warning),
    ).map(Fuse);
    let _log = Logger::root(
        Arc::new(drain),
        o!("version" => env!("CARGO_PKG_VERSION")),
    );
}

#[test]
fn simple_logger_erased() {
    use ::*;

    fn takes_arced_drain(_l: Logger) {}

    let drain = Discard.filter_level(Level::Warning).map(Fuse);
    let log =
        Logger::root_typed(drain, o!("version" => env!("CARGO_PKG_VERSION")));

    takes_arced_drain(log.to_erased());
}

#[test]
fn logger_to_erased() {
    use ::*;

    fn takes_arced_drain(_l: Logger) {}

    let drain = Duplicate(
        Discard.filter(|r| r.level().is_at_least(Level::Info)),
        Discard.filter_level(Level::Warning),
    ).map(Fuse);
    let log =
        Logger::root_typed(drain, o!("version" => env!("CARGO_PKG_VERSION")));

    takes_arced_drain(log.into_erased());
}

#[test]
fn logger_by_ref() {
    use ::*;
    let drain = Discard.filter_level(Level::Warning).map(Fuse);
    let log = Logger::root_typed(drain, o!("version" => env!("CARGO_PKG_VERSION")));
    let f = "f";
    let d = (1, 2);
    info!(&log, "message"; "f" => %f, "d" => ?d);
}

