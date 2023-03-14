#!/bin/sh

set -ex

rm -f Cargo.lock
cargo build

if [ "$RUST_VERSION" = 1.31.0 ] ; then
	exit
fi

# Allow some warnings on the very old compiler.
export RUSTFLAGS="-D warnings"

cargo test --release --all-features
cargo test --release --all-features -- --ignored
