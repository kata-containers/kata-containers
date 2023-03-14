#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.pass("tests/run-pass/*.rs");
    t.compile_fail("tests/compile-fail/*.rs");
}
