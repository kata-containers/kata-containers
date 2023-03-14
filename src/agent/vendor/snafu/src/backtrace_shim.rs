use backtrace;
use std::{fmt, path};

/// A backtrace starting from the beginning of the thread.
///
/// Backtrace functionality is currently **enabled**. Please review
/// [the feature flags](crate::guide::feature_flags) to disable it.
#[derive(Debug)]
pub struct Backtrace(backtrace::Backtrace);

impl crate::GenerateImplicitData for Backtrace {
    // Inlining in an attempt to remove this function from the backtrace
    #[inline(always)]
    fn generate() -> Self {
        Backtrace(backtrace::Backtrace::new())
    }
}

impl crate::AsBacktrace for Backtrace {
    fn as_backtrace(&self) -> Option<&Backtrace> {
        Some(self)
    }
}

impl fmt::Display for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let frames = self.0.frames();
        let width = (frames.len() as f32).log10().floor() as usize + 1;

        for (index, frame) in frames.iter().enumerate() {
            let mut symbols = frame.symbols().iter().map(SymbolDisplay);

            if let Some(symbol) = symbols.next() {
                writeln!(
                    f,
                    "{index:width$} {name}",
                    index = index,
                    width = width,
                    name = symbol.name()
                )?;
                if let Some(location) = symbol.location() {
                    writeln!(
                        f,
                        "{index:width$} {location}",
                        index = "",
                        width = width,
                        location = location
                    )?;
                }

                for symbol in symbols {
                    writeln!(
                        f,
                        "{index:width$} {name}",
                        index = "",
                        width = width,
                        name = symbol.name()
                    )?;
                    if let Some(location) = symbol.location() {
                        writeln!(
                            f,
                            "{index:width$} {location}",
                            index = "",
                            width = width,
                            location = location
                        )?;
                    }
                }
            }
        }

        Ok(())
    }
}

struct SymbolDisplay<'a>(&'a backtrace::BacktraceSymbol);

impl<'a> SymbolDisplay<'a> {
    fn name(&self) -> SymbolNameDisplay<'a> {
        SymbolNameDisplay(self.0)
    }

    fn location(&self) -> Option<SymbolLocationDisplay<'a>> {
        self.0.filename().map(|f| SymbolLocationDisplay(self.0, f))
    }
}

struct SymbolNameDisplay<'a>(&'a backtrace::BacktraceSymbol);

impl<'a> fmt::Display for SymbolNameDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0.name() {
            Some(n) => write!(f, "{}", n)?,
            None => write!(f, "<unknown>")?,
        }

        Ok(())
    }
}

struct SymbolLocationDisplay<'a>(&'a backtrace::BacktraceSymbol, &'a path::Path);

impl<'a> fmt::Display for SymbolLocationDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.1.display())?;
        if let Some(l) = self.0.lineno() {
            write!(f, ":{}", l)?;
        }

        Ok(())
    }
}
