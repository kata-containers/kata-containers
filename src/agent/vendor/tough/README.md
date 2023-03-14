**tough** is a Rust client for [The Update Framework](https://theupdateframework.github.io/) (TUF) repositories.

For more information see the documentation and [the repository](https://github.com/awslabs/tough).

## Testing

Unit tests are run in the usual manner: `cargo test`.
Integration tests require docker and are disabled by default behind a feature named `integ`.
To run all tests, including integration tests: `cargo test --all-features` or `cargo test --features 'http,integ'`.
