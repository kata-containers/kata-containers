use ::snafu as real_snafu;
use real_snafu::{ensure, Snafu};

// Likely candidates to clash with generated code
mod core {}
mod std {}
mod snafu {}

#[derive(Debug, Snafu)]
enum VariantNamedNone {
    #[snafu(context(suffix(false)))]
    None,
}

#[derive(Debug, Snafu)]
enum VariantNamedSome<T> {
    Some { value: T },
}

#[derive(Debug, Snafu)]
enum VariantNamedOk<T> {
    Ok { value: T },
}

#[derive(Debug, Snafu)]
enum VariantNamedErr<T> {
    Err { value: T },
}

fn _using_ensure() -> Result<u8, VariantNamedNone> {
    ensure!(false, None);
    Ok(0)
}
