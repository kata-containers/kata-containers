#[macro_use]
extern crate failure;

// NOTE:
//
// This test is in a separate file due to the fact that ensure! cannot be used
// from within failure.
//
// (you get: 'macro-expanded `macro_export` macros from the current crate cannot
//           be referred to by absolute paths')

// Encloses an early-returning macro in an IIFE so that we
// can treat it as a Result-returning function.
macro_rules! wrap_early_return {
    ($expr:expr) => {{
        fn func() -> Result<(), failure::Error> {
            let _ = $expr;

            #[allow(unreachable_code)]
            Ok(())
        }
        func().map_err(|e| e.to_string())
    }};
}

#[test]
fn bail() {
    assert_eq!(
        wrap_early_return!(bail!("test")),
        wrap_early_return!(bail!("test",)));
    assert_eq!(
        wrap_early_return!(bail!("test {}", 4)),
        wrap_early_return!(bail!("test {}", 4,)));
}

#[test]
fn ensure() {
    assert_eq!(
        wrap_early_return!(ensure!(false, "test")),
        wrap_early_return!(ensure!(false, "test",)));
    assert_eq!(
        wrap_early_return!(ensure!(false, "test {}", 4)),
        wrap_early_return!(ensure!(false, "test {}", 4,)));
}

#[test]
fn single_arg_ensure() {
    assert_eq!(
        wrap_early_return!(ensure!(false)),
        Err("false".to_string()));
    assert_eq!(
        wrap_early_return!(ensure!(true == false)),
        Err("true == false".to_string()));
    assert_eq!(
        wrap_early_return!(ensure!(4 == 5)),
        Err("4 == 5".to_string()));
}

#[test]
fn format_err() {
    assert_eq!(
        format_err!("test").to_string(),
        format_err!("test",).to_string());
    assert_eq!(
        format_err!("test {}", 4).to_string(),
        format_err!("test {}", 4,).to_string());
}
