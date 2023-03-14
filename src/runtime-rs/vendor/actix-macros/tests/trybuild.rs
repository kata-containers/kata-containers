#[rustversion::stable(1.46)] // MSRV
#[test]
fn compile_macros() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/main-01-basic.rs");
    t.compile_fail("tests/trybuild/main-02-only-async.rs");
    t.pass("tests/trybuild/main-03-fn-params.rs");
    t.pass("tests/trybuild/main-04-system-path.rs");
    t.compile_fail("tests/trybuild/main-05-system-expect-path.rs");
    t.compile_fail("tests/trybuild/main-06-unknown-attr.rs");

    t.pass("tests/trybuild/test-01-basic.rs");
    t.pass("tests/trybuild/test-02-keep-attrs.rs");
    t.compile_fail("tests/trybuild/test-03-only-async.rs");
    t.pass("tests/trybuild/test-04-system-path.rs");
    t.compile_fail("tests/trybuild/test-05-system-expect-path.rs");
    t.compile_fail("tests/trybuild/test-06-unknown-attr.rs");
}
