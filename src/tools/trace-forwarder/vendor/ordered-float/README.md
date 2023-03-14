# Ordered Floats

Provides several wrapper types for Ord and Eq implementations on f64.

## Usage

Use the crates.io repository; add this to your `Cargo.toml` along
with the rest of your dependencies:

```toml
[dependencies]
ordered-float = "1.0"
```

See the [API documentation](https://docs.rs/ordered-float) for further details.

## no_std

To use `ordered_float` without requiring the Rust standard library, disable
the default `std` feature:

```toml
[dependencies]
ordered-float = { version = "1.0", default-features = false }
```

## License

MIT
