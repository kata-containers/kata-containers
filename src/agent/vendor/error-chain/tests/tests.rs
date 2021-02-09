#![allow(dead_code)]

#[macro_use]
extern crate error_chain;

#[test]
fn smoke_test_1() {
    error_chain! {
        types {
            Error, ErrorKind, ResultExt, Result;
        }

        links { }

        foreign_links { }

        errors { }
    };
}

#[test]
fn smoke_test_2() {
    error_chain! {
        types { }

        links { }

        foreign_links { }

        errors { }
    };
}

#[test]
fn smoke_test_3() {
    error_chain! {
        links { }

        foreign_links { }

        errors { }
    };
}

#[test]
fn smoke_test_4() {
    error_chain! {
        links { }

        foreign_links { }

        errors {
            HttpStatus(e: u32) {
                description("http request returned an unsuccessful status code")
                display("http request returned an unsuccessful status code: {}", e)
            }
        }
    };
}

#[test]
fn smoke_test_5() {
    error_chain! {
        types { }

        links { }

        foreign_links { }

        errors {
            HttpStatus(e: u32) {
                description("http request returned an unsuccessful status code")
                display("http request returned an unsuccessful status code: {}", e)
            }
        }
    };
}

#[test]
fn smoke_test_6() {
    error_chain! {
        errors {
            HttpStatus(e: u32) {
                description("http request returned an unsuccessful status code")
                display("http request returned an unsuccessful status code: {}", e)
            }
        }
    };
}

#[test]
fn smoke_test_7() {
    error_chain! {
        types { }

        foreign_links { }

        errors {
            HttpStatus(e: u32) {
                description("http request returned an unsuccessful status code")
                display("http request returned an unsuccessful status code: {}", e)
            }
        }
    };
}

#[test]
fn smoke_test_8() {
    error_chain! {
        types { }

        links { }
        links { }

        foreign_links { }
        foreign_links { }

        errors {
            FileNotFound
            AccessDenied
        }
    };
}

#[test]
fn order_test_1() {
    error_chain! { types { } links { } foreign_links { } errors { } };
}

#[test]
fn order_test_2() {
    error_chain! { links { } types { } foreign_links { } errors { } };
}

#[test]
fn order_test_3() {
    error_chain! { foreign_links { }  links { }  errors { } types { } };
}

#[test]
fn order_test_4() {
    error_chain! { errors { } types { } foreign_links { } };
}

#[test]
fn order_test_5() {
    error_chain! { foreign_links { } types { }  };
}

#[test]
fn order_test_6() {
    error_chain! {
        links { }

        errors {
            HttpStatus(e: u32) {
                description("http request returned an unsuccessful status code")
                display("http request returned an unsuccessful status code: {}", e)
            }
        }


        foreign_links { }
    };
}

#[test]
fn order_test_7() {
    error_chain! {
        links { }

        foreign_links { }

        types {
            Error, ErrorKind, ResultExt, Result;
        }
    };
}

#[test]
fn order_test_8() {
    error_chain! {
        links { }

        foreign_links { }
        foreign_links { }

        types {
            Error, ErrorKind, ResultExt, Result;
        }
    };
}

#[test]
fn empty() {
    error_chain! {};
}

#[test]
#[cfg(feature = "backtrace")]
fn has_backtrace_depending_on_env() {
    use std::path::PathBuf;
    use std::process::Command;

    let cmd_folder = if cfg!(build = "debug") {
        "debug"
    } else if cfg!(build = "release") {
        "release"
    } else {
        panic!("Unknown build config");
    };

    let cmd_path = if cfg!(windows) {
        PathBuf::from(format!(
            "./target/{}/examples/has_backtrace.exe",
            cmd_folder
        ))
    } else {
        PathBuf::from(format!("./target/{}/examples/has_backtrace", cmd_folder))
    };
    let mut cmd = Command::new(cmd_path);

    // missing RUST_BACKTRACE and RUST_BACKTRACE=0
    cmd.env_remove("RUST_BACKTRACE");
    assert_eq!(cmd.status().unwrap().code().unwrap(), 0);

    cmd.env("RUST_BACKTRACE", "0");
    assert_eq!(cmd.status().unwrap().code().unwrap(), 0);

    // RUST_BACKTRACE set to anything but 0
    cmd.env("RUST_BACKTRACE", "yes");
    assert_eq!(cmd.status().unwrap().code().unwrap(), 1);

    cmd.env("RUST_BACKTRACE", "1");
    assert_eq!(cmd.status().unwrap().code().unwrap(), 1);
}

#[test]
fn chain_err() {
    use std::fmt;

    error_chain! {
        foreign_links {
            Fmt(fmt::Error);
        }
        errors {
            Test
        }
    }

    let _: Result<()> = Err(fmt::Error).chain_err(|| "");
    let _: Result<()> = Err(Error::from_kind(ErrorKind::Test)).chain_err(|| "");
}

/// Verify that an error chain is extended one by `Error::chain_err`, with
/// the new error added to the end.
#[test]
fn error_chain_err() {
    error_chain! {
        errors {
            Test
        }
    }

    let base = Error::from(ErrorKind::Test);
    let ext = base.chain_err(|| "Test passes");

    if let Error(ErrorKind::Msg(_), _) = ext {
        // pass
    } else {
        panic!("The error should be wrapped. {:?}", ext);
    }
}

#[test]
fn links() {
    mod test {
        error_chain! {}
    }

    error_chain! {
        links {
            Test(test::Error, test::ErrorKind);
        }
    }
}

#[cfg(test)]
mod foreign_link_test {

    use std::fmt;

    // Note: foreign errors must be `pub` because they appear in the
    // signature of the public foreign_link_error_path
    #[derive(Debug)]
    pub struct ForeignError {
        cause: ForeignErrorCause,
    }

    impl ::std::error::Error for ForeignError {
        fn description(&self) -> &'static str {
            "Foreign error description"
        }

        #[cfg(not(has_error_source))]
        fn cause(&self) -> Option<&::std::error::Error> {
            Some(&self.cause)
        }

        #[cfg(has_error_source)]
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.cause)
        }
    }

    impl fmt::Display for ForeignError {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "Foreign error display")
        }
    }

    #[derive(Debug)]
    pub struct ForeignErrorCause {}

    impl ::std::error::Error for ForeignErrorCause {
        fn description(&self) -> &'static str {
            "Foreign error cause description"
        }

        #[cfg(not(has_error_source))]
        fn cause(&self) -> Option<&::std::error::Error> {
            None
        }

        #[cfg(has_error_source)]
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            None
        }
    }

    impl fmt::Display for ForeignErrorCause {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "Foreign error cause display")
        }
    }

    error_chain! {
        types{
            Error, ErrorKind, ResultExt, Result;
        }
        links {}
        foreign_links {
            Foreign(ForeignError);
            Io(::std::io::Error);
        }
        errors {}
    }

    #[test]
    fn display_underlying_error() {
        let chained_error = try_foreign_error().err().unwrap();
        assert_eq!(
            format!(
                "{}",
                ForeignError {
                    cause: ForeignErrorCause {}
                }
            ),
            format!("{}", chained_error)
        );
    }

    #[test]
    #[cfg(not(has_error_source))]
    fn finds_cause() {
        let chained_error = try_foreign_error().err().unwrap();
        assert_eq!(
            format!("{}", ForeignErrorCause {}),
            format!("{}", ::std::error::Error::cause(&chained_error).unwrap())
        );
    }

    #[test]
    #[cfg(has_error_source)]
    fn finds_source() {
        let chained_error = try_foreign_error().err().unwrap();
        assert_eq!(
            format!("{}", ForeignErrorCause {}),
            format!("{}", ::std::error::Error::source(&chained_error).unwrap())
        );
    }

    #[test]
    #[allow(unknown_lints, bare_trait_objects)]
    fn iterates() {
        let chained_error = try_foreign_error().err().unwrap();
        let mut error_iter = chained_error.iter();
        assert!(!format!("{:?}", error_iter).is_empty());
        assert_eq!(
            format!(
                "{}",
                ForeignError {
                    cause: ForeignErrorCause {}
                }
            ),
            format!("{}", error_iter.next().unwrap())
        );
        assert_eq!(
            format!("{}", ForeignErrorCause {}),
            format!("{}", error_iter.next().unwrap())
        );
        assert_eq!(
            format!("{:?}", None as Option<&::std::error::Error>),
            format!("{:?}", error_iter.next())
        );
    }

    fn try_foreign_error() -> Result<()> {
        Err(ForeignError {
            cause: ForeignErrorCause {},
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod attributes_test {
    #[allow(unused_imports)]
    use std::io;

    #[cfg(not(test))]
    mod inner {
        error_chain! {}
    }

    error_chain! {
        types {
            Error, ErrorKind, ResultExt, Result;
        }

        links {
            Inner(inner::Error, inner::ErrorKind) #[cfg(not(test))];
        }

        foreign_links {
            Io(io::Error) #[cfg(not(test))];
        }

        errors {
            #[cfg(not(test))]
            AnError {

            }
        }
    }
}

#[test]
fn with_result() {
    error_chain! {
        types {
            Error, ErrorKind, ResultExt, Result;
        }
    }
    let _: Result<()> = Ok(());
}

#[test]
fn without_result() {
    error_chain! {
        types {
            Error, ErrorKind, ResultExt;
        }
    }
    let _: Result<(), ()> = Ok(());
}

#[test]
fn documentation() {
    mod inner {
        error_chain! {}
    }

    error_chain! {
        links {
            Inner(inner::Error, inner::ErrorKind) #[doc = "Doc"];
        }
        foreign_links {
            Io(::std::io::Error) #[doc = "Doc"];
        }
        errors {
            /// Doc
            Variant
        }
    }
}

#[cfg(test)]
mod multiple_error_same_mod {
    error_chain! {
        types {
            MyError, MyErrorKind, MyResultExt, MyResult;
        }
    }
    error_chain! {}
}

#[doc(test)]
#[deny(dead_code)]
mod allow_dead_code {
    error_chain! {}
}

// Make sure links actually work!
#[test]
fn rustup_regression() {
    error_chain! {
        links {
            Download(error_chain::mock::Error, error_chain::mock::ErrorKind);
        }

        foreign_links { }

        errors {
            LocatingWorkingDir {
                description("could not locate working directory")
            }
        }
    }
}

#[test]
fn error_patterns() {
    error_chain! {
        links { }

        foreign_links { }

        errors { }
    }

    // Tuples look nice when matching errors
    match Error::from("Test") {
        Error(ErrorKind::Msg(_), _) => {}
        _ => {}
    }
}

#[test]
fn result_match() {
    error_chain! {}

    fn ok() -> Result<()> {
        Ok(())
    }

    match ok() {
        Ok(()) => {}
        Err(Error(ErrorKind::Msg(_), _)) => {}
        Err(..) => {}
    }
}

#[test]
fn error_first() {
    error_chain! {
        errors {
            LocatingWorkingDir {
                description("could not locate working directory")
            }
        }

        links {
            Download(error_chain::mock::Error, error_chain::mock::ErrorKind);
        }

        foreign_links { }
    }
}

#[test]
fn bail() {
    error_chain! {
        errors { Foo }
    }

    fn foo() -> Result<()> {
        bail!(ErrorKind::Foo)
    }

    fn bar() -> Result<()> {
        bail!("bar")
    }

    fn baz() -> Result<()> {
        bail!("{}", "baz")
    }
}

#[test]
fn ensure() {
    error_chain! {
        errors { Bar }
    }

    fn foo(x: u8) -> Result<()> {
        ensure!(x == 42, ErrorKind::Bar);
        Ok(())
    }

    assert!(foo(42).is_ok());
    assert!(foo(0).is_err());
}

/// Since the `types` declaration is a list of symbols, check if we
/// don't change their meaning or order.
#[test]
fn types_declarations() {
    error_chain! {
        types {
            MyError, MyErrorKind, MyResultExt, MyResult;
        }
    }

    MyError::from_kind(MyErrorKind::Msg("".into()));

    let err: Result<(), ::std::io::Error> = Ok(());
    MyResultExt::chain_err(err, || "").unwrap();

    let _: MyResult<()> = Ok(());
}

#[test]
/// Calling chain_err over a `Result` containing an error to get a chained error
/// and constructing a MyError directly, passing it an error should be equivalent.
fn rewrapping() {
    use std::env::VarError::{self, NotPresent, NotUnicode};

    error_chain! {
        foreign_links {
            VarErr(VarError);
        }

        types {
            MyError, MyErrorKind, MyResultExt, MyResult;
        }
    }

    let result_a_from_func: Result<String, _> = Err(VarError::NotPresent);
    let result_b_from_func: Result<String, _> = Err(VarError::NotPresent);

    let our_error_a = result_a_from_func.map_err(|e| match e {
        NotPresent => MyError::with_chain(e, "env var wasn't provided"),
        NotUnicode(_) => MyError::with_chain(e, "env var was borkæ–‡å­—åŒ–ã"),
    });

    let our_error_b = result_b_from_func.or_else(|e| match e {
        NotPresent => Err(e).chain_err(|| "env var wasn't provided"),
        NotUnicode(_) => Err(e).chain_err(|| "env var was borkæ–‡å­—åŒ–ã"),
    });

    assert_eq!(
        format!("{}", our_error_a.unwrap_err()),
        format!("{}", our_error_b.unwrap_err())
    );
}

#[test]
fn comma_in_errors_impl() {
    error_chain! {
        links { }

        foreign_links { }

        errors {
            HttpStatus(e: u32) {
                description("http request returned an unsuccessful status code"),
                display("http request returned an unsuccessful status code: {}", e)
            }
        }
    };
}

#[test]
fn trailing_comma_in_errors_impl() {
    error_chain! {
        links { }

        foreign_links { }

        errors {
            HttpStatus(e: u32) {
                description("http request returned an unsuccessful status code"),
                display("http request returned an unsuccessful status code: {}", e),
            }
        }
    };
}

#[test]
fn skipping_msg_variant() {
    error_chain! {
        skip_msg_variant

        errors {
            MyMsg(s: String) {
                description(&s)
                display("{}", s)
            }
        }
    }

    let x = Error::from_kind(ErrorKind::MyMsg("some string".into()));
    // This would fail to compile if we generate a `Msg` variant
    match *x.kind() {
        ErrorKind::MyMsg(_) => {}
        ErrorKind::__Nonexhaustive {} => {}
    }
}
