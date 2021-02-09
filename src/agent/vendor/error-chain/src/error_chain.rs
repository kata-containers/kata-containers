#[doc(hidden)]
#[macro_export]
#[cfg(not(has_error_source))]
macro_rules! impl_error_chain_cause_or_source {
    (
        types {
            $error_kind_name:ident
        }

        foreign_links {
            $( $foreign_link_variant:ident ( $foreign_link_error_path:path )
               $( #[$meta_foreign_links:meta] )*; )*
        }
    ) => {
        #[allow(unknown_lints, renamed_and_removed_lints)]
        #[allow(unused_doc_comment, unused_doc_comments)]
        fn cause(&self) -> Option<&::std::error::Error> {
            match self.1.next_error {
                Some(ref c) => Some(&**c),
                None => {
                    match self.0 {
                        $(
                            $(#[$meta_foreign_links])*
                            $error_kind_name::$foreign_link_variant(ref foreign_err) => {
                                foreign_err.cause()
                            }
                        ) *
                        _ => None
                    }
                }
            }
        }
    };
}

#[cfg(has_error_source)]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_error_chain_cause_or_source {
    (
        types {
             $error_kind_name:ident
        }

        foreign_links {
            $( $foreign_link_variant:ident ( $foreign_link_error_path:path )
               $( #[$meta_foreign_links:meta] )*; )*
        }
    ) => {
            #[allow(unknown_lints, renamed_and_removed_lints, bare_trait_objects)]
            #[allow(unused_doc_comment, unused_doc_comments)]
            fn source(&self) -> Option<&(std::error::Error + 'static)> {
                match self.1.next_error {
                    Some(ref c) => Some(&**c),
                    None => {
                        match self.0 {
                        $(
                            $(#[$meta_foreign_links])*
                            $error_kind_name::$foreign_link_variant(ref foreign_err) => {
                                foreign_err.source()
                            }
                        ) *
                            _ => None
                        }
                    }
                }
            }
        };
}

/// Conditional usage of deprecated Error::description
#[doc(hidden)]
#[cfg(has_error_description_deprecated)]
#[macro_export(local_inner_macros)]
macro_rules! call_to_deprecated_description {
    ($e:ident) => {
        ""
    };
}

#[doc(hidden)]
#[cfg(not(has_error_description_deprecated))]
#[macro_export(local_inner_macros)]
macro_rules! call_to_deprecated_description {
    ($e:ident) => {
        ::std::error::Error::description($e)
    };
}

/// Prefer to use `error_chain` instead of this macro.
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! impl_error_chain_processed {
    // Default values for `types`.
    (
        types {}
        $( $rest: tt )*
    ) => {
        impl_error_chain_processed! {
            types {
                Error, ErrorKind, ResultExt, Result;
            }
            $( $rest )*
        }
    };
    // With `Result` wrapper.
    (
        types {
            $error_name:ident, $error_kind_name:ident,
            $result_ext_name:ident, $result_name:ident;
        }
        $( $rest: tt )*
    ) => {
        impl_error_chain_processed! {
            types {
                $error_name, $error_kind_name,
                $result_ext_name;
            }
            $( $rest )*
        }
        /// Convenient wrapper around `std::Result`.
        #[allow(unused)]
        pub type $result_name<T> = ::std::result::Result<T, $error_name>;
    };

    // With `Msg` variant.
    (
        types {
            $error_name:ident, $error_kind_name:ident, $($types:tt)*
        }
        links $links:tt
        foreign_links $foreign_links:tt
        errors { $($errors:tt)* }
    ) => {
        impl_error_chain_processed! {
            types {
                $error_name, $error_kind_name, $($types)*
            }
            skip_msg_variant
            links $links
            foreign_links $foreign_links
            errors {
                /// A convenient variant for String.
                Msg(s: String) {
                    description(&s)
                    display("{}", s)
                }

                $($errors)*
            }
        }

        impl<'a> From<&'a str> for $error_kind_name {
            fn from(s: &'a str) -> Self {
                $error_kind_name::Msg(s.into())
            }
        }

        impl From<String> for $error_kind_name {
            fn from(s: String) -> Self {
                $error_kind_name::Msg(s)
            }
        }

        impl<'a> From<&'a str> for $error_name {
            fn from(s: &'a str) -> Self {
                Self::from_kind(s.into())
            }
        }

        impl From<String> for $error_name {
            fn from(s: String) -> Self {
                Self::from_kind(s.into())
            }
        }
    };

    // Without `Result` wrapper or `Msg` variant.
    (
        types {
            $error_name:ident, $error_kind_name:ident,
            $result_ext_name:ident;
        }

        skip_msg_variant

        links {
            $( $link_variant:ident ( $link_error_path:path, $link_kind_path:path )
               $( #[$meta_links:meta] )*; ) *
        }

        foreign_links {
            $( $foreign_link_variant:ident ( $foreign_link_error_path:path )
               $( #[$meta_foreign_links:meta] )*; )*
        }

        errors {
            $( $error_chunks:tt ) *
        }

    ) => {
        /// The Error type.
        ///
        /// This tuple struct is made of two elements:
        ///
        /// - an `ErrorKind` which is used to determine the type of the error.
        /// - An internal `State`, not meant for direct use outside of `error_chain`
        ///   internals, containing:
        ///   - a backtrace, generated when the error is created.
        ///   - an error chain, used for the implementation of `Error::cause()`.
        #[derive(Debug)]
        pub struct $error_name(
            // The members must be `pub` for `links`.
            /// The kind of the error.
            pub $error_kind_name,
            /// Contains the error chain and the backtrace.
            #[doc(hidden)]
            pub $crate::State,
        );

        impl $crate::ChainedError for $error_name {
            type ErrorKind = $error_kind_name;

            fn new(kind: $error_kind_name, state: $crate::State) -> $error_name {
                $error_name(kind, state)
            }

            fn from_kind(kind: Self::ErrorKind) -> Self {
                Self::from_kind(kind)
            }

            fn with_chain<E, K>(error: E, kind: K)
                -> Self
                where E: ::std::error::Error + Send + 'static,
                      K: Into<Self::ErrorKind>
            {
                Self::with_chain(error, kind)
            }

            fn kind(&self) -> &Self::ErrorKind {
                self.kind()
            }

            fn iter(&self) -> $crate::Iter {
                $crate::Iter::new(Some(self))
            }

            fn chain_err<F, EK>(self, error: F) -> Self
                where F: FnOnce() -> EK,
                      EK: Into<$error_kind_name> {
                self.chain_err(error)
            }

            fn backtrace(&self) -> Option<&$crate::Backtrace> {
                self.backtrace()
            }

            impl_extract_backtrace!($error_name
                                    $error_kind_name
                                    $([$link_error_path, $(#[$meta_links])*])*);
        }

        #[allow(dead_code)]
        impl $error_name {
            /// Constructs an error from a kind, and generates a backtrace.
            pub fn from_kind(kind: $error_kind_name) -> $error_name {
                $error_name(
                    kind,
                    $crate::State::default(),
                )
            }

            /// Constructs a chained error from another error and a kind, and generates a backtrace.
            pub fn with_chain<E, K>(error: E, kind: K)
                -> $error_name
                where E: ::std::error::Error + Send + 'static,
                      K: Into<$error_kind_name>
            {
                $error_name::with_boxed_chain(Box::new(error), kind)
            }

            /// Construct a chained error from another boxed error and a kind, and generates a backtrace
            #[allow(unknown_lints, bare_trait_objects)]
            pub fn with_boxed_chain<K>(error: Box<::std::error::Error + Send>, kind: K)
                -> $error_name
                where K: Into<$error_kind_name>
            {
                $error_name(
                    kind.into(),
                    $crate::State::new::<$error_name>(error, ),
                )
            }

            /// Returns the kind of the error.
            pub fn kind(&self) -> &$error_kind_name {
                &self.0
            }

            /// Iterates over the error chain.
            pub fn iter(&self) -> $crate::Iter {
                $crate::ChainedError::iter(self)
            }

            /// Returns the backtrace associated with this error.
            pub fn backtrace(&self) -> Option<&$crate::Backtrace> {
                self.1.backtrace()
            }

            /// Extends the error chain with a new entry.
            pub fn chain_err<F, EK>(self, error: F) -> $error_name
                where F: FnOnce() -> EK, EK: Into<$error_kind_name> {
                $error_name::with_chain(self, Self::from_kind(error().into()))
            }

            /// A short description of the error.
            /// This method is identical to [`Error::description()`](https://doc.rust-lang.org/nightly/std/error/trait.Error.html#tymethod.description)
            pub fn description(&self) -> &str {
                self.0.description()
            }
        }

        impl ::std::error::Error for $error_name {
            #[cfg(not(has_error_description_deprecated))]
            fn description(&self) -> &str {
                self.description()
            }

            impl_error_chain_cause_or_source!{
                types {
                    $error_kind_name
                }
                foreign_links {
                    $( $foreign_link_variant ( $foreign_link_error_path )
                    $( #[$meta_foreign_links] )*; )*
                }
            }
        }

        impl ::std::fmt::Display for $error_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        $(
            $(#[$meta_links])*
            impl From<$link_error_path> for $error_name {
                fn from(e: $link_error_path) -> Self {
                    $error_name(
                        $error_kind_name::$link_variant(e.0),
                        e.1,
                    )
                }
            }
        ) *

        $(
            $(#[$meta_foreign_links])*
            impl From<$foreign_link_error_path> for $error_name {
                fn from(e: $foreign_link_error_path) -> Self {
                    $error_name::from_kind(
                        $error_kind_name::$foreign_link_variant(e)
                    )
                }
            }
        ) *

        impl From<$error_kind_name> for $error_name {
            fn from(e: $error_kind_name) -> Self {
                $error_name::from_kind(e)
            }
        }

        // The ErrorKind type
        // --------------

        impl_error_chain_kind! {
            /// The kind of an error.
            #[derive(Debug)]
            pub enum $error_kind_name {
                $(
                    $(#[$meta_links])*
                    $link_variant(e: $link_kind_path) {
                        description(e.description())
                        display("{}", e)
                    }
                ) *

                $(
                    $(#[$meta_foreign_links])*
                    $foreign_link_variant(err: $foreign_link_error_path) {
                        description(call_to_deprecated_description!(err))
                        display("{}", err)
                    }
                ) *

                $($error_chunks)*
            }
        }

        $(
            $(#[$meta_links])*
            impl From<$link_kind_path> for $error_kind_name {
                fn from(e: $link_kind_path) -> Self {
                    $error_kind_name::$link_variant(e)
                }
            }
        ) *

        impl From<$error_name> for $error_kind_name {
            fn from(e: $error_name) -> Self {
                e.0
            }
        }

        // The ResultExt trait defines the `chain_err` method.

        /// Additional methods for `Result`, for easy interaction with this crate.
        pub trait $result_ext_name<T> {
            /// If the `Result` is an `Err` then `chain_err` evaluates the closure,
            /// which returns *some type that can be converted to `ErrorKind`*, boxes
            /// the original error to store as the cause, then returns a new error
            /// containing the original error.
            fn chain_err<F, EK>(self, callback: F) -> ::std::result::Result<T, $error_name>
                where F: FnOnce() -> EK,
                      EK: Into<$error_kind_name>;
        }

        impl<T, E> $result_ext_name<T> for ::std::result::Result<T, E> where E: ::std::error::Error + Send + 'static {
            fn chain_err<F, EK>(self, callback: F) -> ::std::result::Result<T, $error_name>
                where F: FnOnce() -> EK,
                      EK: Into<$error_kind_name> {
                self.map_err(move |e| {
                    let state = $crate::State::new::<$error_name>(Box::new(e), );
                    $crate::ChainedError::new(callback().into(), state)
                })
            }
        }

        impl<T> $result_ext_name<T> for ::std::option::Option<T> {
            fn chain_err<F, EK>(self, callback: F) -> ::std::result::Result<T, $error_name>
                where F: FnOnce() -> EK,
                      EK: Into<$error_kind_name> {
                self.ok_or_else(move || {
                    $crate::ChainedError::from_kind(callback().into())
                })
            }
        }


    };
}

/// Internal macro used for reordering of the fields.
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! error_chain_processing {
    (
        ({}, $($rest:tt)*)
        types $content:tt
        $( $tail:tt )*
    ) => {
        error_chain_processing! {
            ($content, $($rest)*)
            $($tail)*
        }
    };

    (
        ($a:tt, {}, $($rest:tt)*)
        links $content:tt
        $( $tail:tt )*
    ) => {
        error_chain_processing! {
            ($a, $content, $($rest)*)
            $($tail)*
        }
    };

    (
        ($a:tt, $b:tt, {}, $($rest:tt)*)
        foreign_links $content:tt
        $( $tail:tt )*
    ) => {
        error_chain_processing! {
            ($a, $b, $content, $($rest)*)
            $($tail)*
        }
    };

    (
        ($a:tt, $b:tt, $c:tt, {}, $($rest:tt)*)
        errors $content:tt
        $( $tail:tt )*
    ) => {
        error_chain_processing! {
            ($a, $b, $c, $content, $($rest)*)
            $($tail)*
        }
    };

    (
        ($a:tt, $b:tt, $c:tt, $d:tt, {}, $($rest:tt)*)
        skip_msg_variant
        $( $tail:tt )*
    ) => {
        error_chain_processing! {
            ($a, $b, $c, $d, {skip_msg_variant}, $($rest)*)
            $($tail)*
        }
    };

    ( ($a:tt, $b:tt, $c:tt, $d:tt, {$($e:tt)*},) ) => {
        impl_error_chain_processed! {
            types $a
            $($e)*
            links $b
            foreign_links $c
            errors $d
        }
    };
}

/// Macro for generating error types and traits. See crate level documentation for details.
#[macro_export(local_inner_macros)]
macro_rules! error_chain {
    ( $($args:tt)* ) => {
        error_chain_processing! {
            ({}, {}, {}, {}, {},)
            $($args)*
        }
    };
}

/// Macro used to manage the `backtrace` feature.
///
/// See
/// https://www.reddit.com/r/rust/comments/57virt/hey_rustaceans_got_an_easy_question_ask_here/da5r4ti/?context=3
/// for more details.
#[macro_export]
#[doc(hidden)]
macro_rules! impl_extract_backtrace {
    ($error_name: ident
     $error_kind_name: ident
     $([$link_error_path: path, $(#[$meta_links: meta])*])*) => {
        #[allow(unknown_lints, renamed_and_removed_lints, bare_trait_objects)]
        #[allow(unused_doc_comment, unused_doc_comments)]
        fn extract_backtrace(e: &(::std::error::Error + Send + 'static))
            -> Option<$crate::InternalBacktrace> {
            if let Some(e) = e.downcast_ref::<$error_name>() {
                return Some(e.1.backtrace.clone());
            }
            $(
                $( #[$meta_links] )*
                {
                    if let Some(e) = e.downcast_ref::<$link_error_path>() {
                        return Some(e.1.backtrace.clone());
                    }
                }
            ) *
            None
        }
    }
}
