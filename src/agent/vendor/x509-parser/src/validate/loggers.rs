pub trait Logger {
    fn warn(&mut self, message: &str);

    fn err(&mut self, message: &str);
}

/// Simple Logger for [`Validator`](crate::validate::Validator) trait, storing messages in `Vec`
#[derive(Debug, Default)]
pub struct VecLogger {
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl VecLogger {
    /// Get stored warnings
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Get stored errors
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

impl Logger for VecLogger {
    fn warn(&mut self, message: &str) {
        self.warnings.push(message.to_owned())
    }

    fn err(&mut self, message: &str) {
        self.errors.push(message.to_owned())
    }
}

/// Simple Logger for [`Validator`](crate::validate::Validator) trait, printing messages to `stderr`
#[derive(Debug, Default)]
pub struct StderrLogger;

impl Logger for StderrLogger {
    fn warn(&mut self, message: &str) {
        eprintln!("[W] {}", message);
    }

    fn err(&mut self, message: &str) {
        eprintln!("[E] {}", message);
    }
}

/// Simple Logger for [`Validator`](crate::validate::Validator) trait, using closures for `warn`/`err`.
#[derive(Debug, Default)]
pub struct CallbackLogger<W, E>
where
    W: FnMut(&str),
    E: FnMut(&str),
{
    warn: W,
    err: E,
}

impl<W, E> CallbackLogger<W, E>
where
    W: FnMut(&str),
    E: FnMut(&str),
{
    pub fn new(warn: W, err: E) -> Self {
        CallbackLogger { warn, err }
    }
}

impl<W, E> Logger for CallbackLogger<W, E>
where
    W: FnMut(&str),
    E: FnMut(&str),
{
    fn warn(&mut self, message: &str) {
        (self.warn)(message);
    }

    fn err(&mut self, message: &str) {
        (self.err)(message);
    }
}
