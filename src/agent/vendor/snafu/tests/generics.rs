use snafu::{prelude::*, Backtrace};

type BoxError = Box<dyn std::error::Error>;

#[derive(Debug, Snafu)]
enum Error<'a, 'x, A, Y> {
    Everything {
        source: BoxError,
        name: &'a str,
        length: A,
        backtrace: Backtrace,
    },
    Lifetime {
        key: &'x i32,
    },
    Type {
        value: Y,
    },
}

fn cause_error() -> Result<(), BoxError> {
    Ok(())
}

fn example<'s, 'k, V>(name: &'s str, key: &'k i32, value: V) -> Result<(), Error<'s, 'k, usize, V>>
where
    V: std::fmt::Debug,
{
    let length = name.len();

    cause_error().context(EverythingSnafu { name, length })?;

    if name == "alice" {
        return LifetimeSnafu { key }.fail();
    }

    if name == "bob" {
        return TypeSnafu { value }.fail();
    }

    Ok(())
}

#[test]
fn implements_error() {
    let name = String::from("hello");
    let key = Box::new(42);
    let value = vec![false];

    example(&name, &key, value).unwrap();
}

mod bounds {
    mod inline {
        use snafu::prelude::*;
        use std::fmt::{Debug, Display};

        #[derive(Debug, Snafu)]
        pub struct ApiError<T: Debug + Display>(Error<T>);

        #[derive(Debug, Snafu)]
        enum Error<T: Display> {
            #[snafu(display("Boom: {}", value))]
            Boom { value: T },
        }

        #[test]
        fn implements_error() {
            fn check_bounds<T: std::error::Error>() {}
            check_bounds::<Error<i32>>();
            check_bounds::<ApiError<i32>>();
        }
    }

    mod where_clause {
        use snafu::prelude::*;
        use std::fmt::{Debug, Display};

        #[derive(Debug, Snafu)]
        pub struct ApiError<T>(Error<T>)
        where
            T: Debug + Display;

        #[derive(Debug, Snafu)]
        enum Error<T>
        where
            T: Display,
        {
            #[snafu(display("Boom: {}", value))]
            Boom { value: T },
        }

        #[test]
        fn implements_error() {
            fn check_bounds<T: std::error::Error>() {}
            check_bounds::<Error<i32>>();
            check_bounds::<ApiError<i32>>();
        }
    }
}
