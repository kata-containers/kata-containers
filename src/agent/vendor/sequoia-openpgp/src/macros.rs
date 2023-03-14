use std::cmp;

macro_rules! trace {
    ( $TRACE:expr, $fmt:expr, $($pargs:expr),* ) => {
        if $TRACE {
            eprintln!($fmt, $($pargs),*);
        }
    };
    ( $TRACE:expr, $fmt:expr ) => {
        trace!($TRACE, $fmt, );
    };
}

// Converts an indentation level to whitespace.
pub(crate) fn indent(i: isize) -> &'static str {
    use std::convert::TryFrom;
    let s = "                                                  ";
    &s[0..cmp::min(usize::try_from(i).unwrap_or(0), s.len())]
}

macro_rules! tracer {
    ( $TRACE:expr, $func:expr ) => {
        tracer!($TRACE, $func, 0)
    };
    ( $TRACE:expr, $func:expr, $indent:expr ) => {
        // Currently, Rust doesn't support $( ... ) in a nested
        // macro's definition.  See:
        // https://users.rust-lang.org/t/nested-macros-issue/8348/2
        #[allow(unused_macros)]
        macro_rules! t {
            ( $fmt:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, $fmt) };
            ( $fmt:expr, $a:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a)) };
            ( $fmt:expr, $a:expr, $b:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d, $e)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d, $e, $f)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d, $e, $f, $g)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d, $e, $f, $g, $h)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d, $e, $f, $g, $h, $i)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr, $j:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d, $e, $f, $g, $h, $i, $j)) };
            ( $fmt:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr, $j:expr, $k:expr ) =>
            { trace!($TRACE, "{}{}: {}", crate::macros::indent($indent), $func, format!($fmt, $a, $b, $c, $d, $e, $f, $g, $h, $i, $j, $k)) };
        }
    }
}


/// A very simple profiling tool.
///
/// Note: don't ever profile code that has not been compiled in
/// release mode.  There can be orders of magnitude difference in
/// execution time between it and debug mode!
///
/// This macro measures the wall time it takes to execute the block.
/// If the time is at least $ms_threshold (in milli-seconds), then it
/// displays the output on stderr.  The output is prefixed with label,
/// if it is provided.
///
/// ```ignore
/// let result = time_it!("Some code", 10, {
///     // Some code.
///     5
/// });
/// assert_eq!(result, 5);
/// ```
// Note: We cannot test the macro in doctests, because the macro is
// not public.  We test the cases in the test module below, instead.
// If you change the examples here, propagate the changes to the
// module below.
#[allow(unused_macros)]
macro_rules! time_it {
    ( $label:expr, $ms_threshold:expr, $body:expr ) => {{
        use std::time::{SystemTime, Duration};

        // We use drop so that code that uses non-local exits (e.g.,
        // using break 'label) still works.
        struct Timer {
            start: SystemTime,
        }
        impl Drop for Timer {
            fn drop(&mut self) {
                let elapsed = self.start.elapsed();
                if elapsed.clone().unwrap_or(Duration::from_millis($ms_threshold))
                    >= Duration::from_millis($ms_threshold)
                {
                    if $label.len() > 0 {
                        eprint!("{}:", $label);
                    }
                    eprintln!("{}:{}: {:?}", file!(), line!(), elapsed);
                }
            }
        }

        let _start = Timer { start: SystemTime::now() };
        $body
    }};
    ( $label:expr, $body:expr ) => {
        time_it!($label, 0, $body)
    };
    ( $body:expr ) => {
        time_it!("", $body)
    };
}

/// We cannot test the macro in doctests, because the macro is not
/// public.  We test the cases here, instead.  If you change the
/// examples here, propagate the changes to the docstring above.
#[cfg(test)]
mod test_time_it {
    /// This macro measures the wall time it takes to execute the
    /// block.  If the time is at least $ms_threshold (in
    /// milli-seconds), then it displays the output on stderr.  The
    /// output is prefixed with label, if it is provided.
    #[test]
    fn time_it() {
        let result = time_it!("Some code", 10, {
            // Some code.
            5
        });
        assert_eq!(result, 5);
    }
}

/// A simple shortcut for ensuring a type is send and sync.
///
/// For most types just call it after defining the type:
///
/// ```ignore
/// pub struct MyStruct {}
/// assert_send_and_sync!(MyStruct);
/// ```
///
/// For types with lifetimes, use the anonymous lifetime:
///
/// ```ignore
/// pub struct WithLifetime<'a> { _p: std::marker::PhantomData<&'a ()> }
/// assert_send_and_sync!(WithLifetime<'_>);
/// ```
///
/// For a type generic over another type `W`,
/// pass the type `W` as a where clause
/// including a trait bound when needed:
///
/// ```ignore
/// pub struct MyWriter<W: std::io::Write> { _p: std::marker::PhantomData<W> }
/// assert_send_and_sync!(MyWriter<W> where W: std::io::Write);
/// ```
///
/// This will assert that `MyWriterStruct<W>` is `Send` and `Sync`
/// if `W` is `Send` and `Sync`.
///
/// You can also combine the two and be generic over multiple types.
/// Just make sure to list all the types - even those without additional
/// trait bounds:
///
/// ```ignore
/// pub struct MyWriterWithLifetime<'a, C, W: std::io::Write> {
///     _p: std::marker::PhantomData<&'a (C, W)>,
/// }
/// assert_send_and_sync!(MyWriterWithLifetime<'_, C, W> where C, W: std::io::Write);
/// ```
///
/// If you need multiple additional trait bounds on a single type
/// you can add them separated by `+` like in normal where clauses.
/// However you have to make sure they are `Identifiers` like `Write`.
/// In macro patterns `Paths` (like `std::io::Write`) may not be followed
/// by `+` characters.
// Note: We cannot test the macro in doctests, because the macro is
// not public.  We test the cases in the test module below, instead.
// If you change the examples here, propagate the changes to the
// module below.
macro_rules! assert_send_and_sync {
    ( $x:ty where $( $g:ident$( : $a:path )? $(,)?)*) => {
        impl<$( $g ),*> crate::types::Sendable for $x
            where $( $g: Send + Sync $( + $a )? ),*
            {}
        impl<$( $g ),*> crate::types::Syncable for $x
            where $( $g: Send + Sync $( + $a )? ),*
            {}
    };
    ( $x:ty where $( $g:ident$( : $a:ident $( + $b:ident )* )? $(,)?)*) => {
        impl<$( $g ),*> crate::types::Sendable for $x
            where $( $g: Send + Sync $( + $a $( + $b )* )? ),*
            {}
        impl<$( $g ),*> crate::types::Syncable for $x
            where $( $g: Send + Sync $( + $a $( + $b )* )? ),*
            {}
    };
    ( $x:ty ) => {
        impl crate::types::Sendable for $x {}
        impl crate::types::Syncable for $x {}
    };
}

/// We cannot test the macro in doctests, because the macro is not
/// public.  We test the cases here, instead.  If you change the
/// examples here, propagate the changes to the docstring above.
#[cfg(test)]
mod test_assert_send_and_sync {
    /// For most types just call it after defining the type:
    pub struct MyStruct {}
    assert_send_and_sync!(MyStruct);

    /// For types with lifetimes, use the anonymous lifetime:
    pub struct WithLifetime<'a> { _p: std::marker::PhantomData<&'a ()> }
    assert_send_and_sync!(WithLifetime<'_>);

    /// For a type generic over another type `W`, pass the type `W` as
    /// a where clause including a trait bound when needed:
    pub struct MyWriter<W: std::io::Write> { _p: std::marker::PhantomData<W> }
    assert_send_and_sync!(MyWriter<W> where W: std::io::Write);

    /// This will assert that `MyWriterStruct<W>` is `Send` and `Sync`
    /// if `W` is `Send` and `Sync`.
    ///
    /// You can also combine the two and be generic over multiple
    /// types.  Just make sure to list all the types - even those
    /// without additional trait bounds:
    pub struct MyWriterWithLifetime<'a, C, W: std::io::Write> {
        _p: std::marker::PhantomData<&'a (C, W)>,
    }
    assert_send_and_sync!(MyWriterWithLifetime<'_, C, W> where C, W: std::io::Write);
}
