#[test]
fn compile_test_success() {
    let t = trybuild::TestCases::new();
    t.pass("tests/run-pass/*.rs");
}

#[rustversion::any(nightly)]
#[test]
fn compile_fail_test_prerelease() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail_nightly/*.rs");
}

#[rustversion::before(1.46)] // nightly+beta has changed the format of the macro backtrace hint.
#[test]
fn compile_fail_test_until_1_45() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail_1.45/*.rs");
}

#[rustversion::all(since(1.46), before(1.48))]
#[test]
fn compile_fail_test_since_1_46() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail_1.46/*.rs");
}

#[rustversion::all(since(1.48), before(1.54.0))]
#[test]
fn compile_fail_test_stable_since_1_48() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail_1.48/*.rs");
}

#[rustversion::all(since(1.54.0), not(nightly))]
#[test]
fn compile_fail_test_since_1_54() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail_1.54/*.rs");
}
