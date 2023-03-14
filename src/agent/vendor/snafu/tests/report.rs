use snafu::{prelude::*, IntoError, Report};

macro_rules! assert_contains {
    (needle: $needle:expr, haystack: $haystack:expr) => {
        assert!(
            $haystack.contains($needle),
            "Expected {:?} to include {:?}",
            $haystack,
            $needle,
        )
    };
}

macro_rules! assert_not_contains {
    (needle: $needle:expr, haystack: $haystack:expr) => {
        assert!(
            !$haystack.contains($needle),
            "Expected {:?} to not include {:?}",
            $haystack,
            $needle,
        )
    };
}

#[test]
fn includes_the_error_display_text() {
    #[derive(Debug, Snafu)]
    #[snafu(display("This is my Display text!"))]
    struct Error;

    let r = Report::from_error(Error);
    let msg = r.to_string();

    let expected = "This is my Display text!";
    assert_contains!(needle: expected, haystack: msg);
}

#[test]
fn includes_the_source_display_text() {
    #[derive(Debug, Snafu)]
    #[snafu(display("This is my inner Display"))]
    struct InnerError;

    #[derive(Debug, Snafu)]
    #[snafu(display("This is my outer Display"))]
    struct OuterError {
        source: InnerError,
    }

    let e = OuterSnafu.into_error(InnerError);
    let r = Report::from_error(e);
    let msg = r.to_string();

    let expected = "This is my inner Display";
    assert_contains!(needle: expected, haystack: msg);
}

#[test]
fn reduces_duplication_of_the_source_display_text() {
    // Including the source in the Display message is discouraged but
    // quite common.

    #[derive(Debug, Snafu)]
    #[snafu(display("Level 0"))]
    struct Level0Error;

    #[derive(Debug, Snafu)]
    #[snafu(display("Level 1: {source}"))]
    struct Level1Error {
        source: Level0Error,
    }

    #[derive(Debug, Snafu)]
    #[snafu(display("Level 2: {source}"))]
    struct Level2Error {
        source: Level1Error,
    }

    let e = Level2Snafu.into_error(Level1Snafu.into_error(Level0Error));
    let raw_msg = e.to_string();

    let expected = "Level 2: Level 1";
    assert_contains!(needle: expected, haystack: raw_msg);

    let r = Report::from_error(e);
    let msg = r.to_string();

    assert_not_contains!(needle: expected, haystack: msg);
}

#[test]
fn removes_complete_duplication_in_the_source_display_text() {
    // Including **only** the source in the Display message is also
    // discouraged but occurs.

    #[derive(Debug, Snafu)]
    #[snafu(display("Level 0"))]
    struct Level0Error;

    #[derive(Debug, Snafu)]
    #[snafu(display("{source}"))]
    struct Level1Error {
        source: Level0Error,
    }

    #[derive(Debug, Snafu)]
    #[snafu(display("{source}"))]
    struct Level2Error {
        source: Level1Error,
    }

    let e = Level2Snafu.into_error(Level1Snafu.into_error(Level0Error));
    let raw_msg = e.to_string();

    assert_contains!(needle: "Level 0", haystack: raw_msg);

    let r = Report::from_error(e);
    let msg = r.to_string();

    assert_not_contains!(needle: "Caused by", haystack: msg);
}

#[test]
fn debug_and_display_are_the_same() {
    #[derive(Debug, Snafu)]
    #[snafu(display("This is my inner Display"))]
    struct InnerError;

    #[derive(Debug, Snafu)]
    #[snafu(display("This is my outer Display"))]
    struct OuterError {
        source: InnerError,
    }

    let e = OuterSnafu.into_error(InnerError);
    let r = Report::from_error(e);

    let display = format!("{}", r);
    let debug = format!("{:?}", r);

    assert_eq!(display, debug);
}

#[test]
fn procedural_macro_works_with_result_return_type() {
    #[derive(Debug, Snafu)]
    struct Error;

    #[snafu::report]
    fn mainlike_result() -> Result<(), Error> {
        Ok(())
    }

    let _: Result<(), Report<Error>> = mainlike_result();
}

#[derive(Debug, Snafu)]
struct TestFunctionError;

#[test]
#[snafu::report]
fn procedural_macro_works_with_test_functions() -> Result<(), TestFunctionError> {
    Ok(())
}
