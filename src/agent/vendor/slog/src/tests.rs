use {Discard, Logger, Never, KV, Drain, OwnedKVList, Record, AsFmtSerializer};

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

    #[derive(Clone)]
    struct CheckError;

    impl Drain for CheckError {
        type Ok = ();
        type Err = Never;
        fn log(
            &self,
            record: &Record,
            values: &OwnedKVList,
        ) -> std::result::Result<Self::Ok, Self::Err> {
            struct ErrorSerializer(String);

            impl Serializer for ErrorSerializer {
                fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> Result {
                    use core::fmt::Write;

                    assert!(key == "error");
                    self.0.write_fmt(*val).unwrap();
                    Ok(())
                }
            }

            let mut serializer = ErrorSerializer(String::new());
            values.serialize(record, &mut serializer).unwrap();
            assert_eq!(
                serializer.0,
                format!("{}", record.msg())
            );
            Ok(())
        }
    }

    #[derive(Debug)]
    struct TestError<E=std::string::ParseError>(&'static str, Option<E>);

    impl TestError {
        fn new(message: &'static str) -> Self {
            TestError(message, None)
        }
    }

    impl<E> fmt::Display for TestError<E> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl<E: std::error::Error + 'static> std::error::Error for TestError<E> {
        #[allow(deprecated)]
        fn cause(&self) -> Option<&std::error::Error> {
            self.1.as_ref().map(|error| error as _)
        }

        #[allow(deprecated)]
        fn description(&self) -> &str {
            "test error"
        }
    }

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

    #[test]
    fn error_fmt_no_source() {
        let logger = Logger::root(CheckError, o!("error" => #TestError::new("foo")));
        info!(logger, "foo");
    }

    #[test]
    fn error_fmt_single_source() {
        let logger = Logger::root(CheckError, o!("error" => #TestError("foo", Some(TestError::new("bar")))));
        info!(logger, "foo: bar");
    }

    #[test]
    fn error_fmt_two_sources() {
        let logger = Logger::root(CheckError, o!("error" => #TestError("foo", Some(TestError("bar", Some(TestError::new("baz")))))));
        info!(logger, "foo: bar: baz");
    }

    #[test]
    fn ioerror_impls_value() {
        let logger = Logger::root(Discard, o!());
        info!(logger, "not found"; "error" => std::io::Error::from(std::io::ErrorKind::NotFound));
        // compiles?
        info!(logger, "not found"; "error" => #std::io::Error::from(std::io::ErrorKind::NotFound));
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

#[cfg(feature = "std")]
#[test]
fn display_and_alternate_display() {
    use core::fmt;
    use core::cell::Cell;

    struct Example;

    impl fmt::Display for Example {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            if f.alternate() {
                f.write_str("alternate")
            } else {
                f.write_str("normal")
            }
        }
    }

    #[derive(Clone, Default)]
    struct CheckExample;

    impl Drain for CheckExample {
        type Ok = ();
        type Err = Never;

        fn log(&self, record: &Record, values: &OwnedKVList) -> Result<(), Never> {
            let mut checked_n = false;
            let mut checked_a = false;
            {
                let mut serializer = AsFmtSerializer(|key, fmt_args| {
                    if key == "n" {
                        assert_eq!(format!("{}", fmt_args), "normal");
                        checked_n = true;
                    } else if key == "a" {
                        assert_eq!(format!("{}", fmt_args), "alternate");
                        checked_a = true;
                    } else {
                        panic!("Unexpected key: {}", key);
                    }
                    Ok(())
                });

                record.kv.serialize(record, &mut serializer).unwrap();
            }

            assert!(checked_n, "Expected the normal formatter to be used");
            assert!(checked_a, "Expected the alternate formatter to be used");

            Ok(())
        }
    }

    let log = Logger::root(CheckExample, o!());

    info!(log, ""; "n" => %Example, "a" => #%Example);
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

#[test]
fn test_never_type_clone() {
    // We just want to make sure that this compiles
    fn _do_not_run() {
        let x: Never = panic!("Can't actually construct a Never type here!");
        let y = x.clone();
    }
    // Always pass if we compiled
}

#[cfg(feature = "std")]
#[test]
fn can_hash_keys() {
    use std::collections::HashSet;
    use Key;
    let tab: HashSet<Key> = ["foo"].iter().map(|&k| k.into()).collect();
}
