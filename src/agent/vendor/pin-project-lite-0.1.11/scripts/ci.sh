#!/bin/bash

# A script to run a simplified version of the checks done by CI.
#
# Usage:
#     bash scripts/ci.sh
#
# Note: This script requires nightly Rust, rustfmt, clippy, and cargo-expand

set -euo pipefail

if [[ "${1:-none}" == "+"* ]]; then
    toolchain="${1}"
else
    cargo +nightly -V >/dev/null || exit 1
    toolchain="+nightly"
fi

if [[ "${toolchain:-+nightly}" != "+nightly"* ]] || ! rustfmt -V &>/dev/null || ! cargo clippy -V &>/dev/null || ! cargo expand -V &>/dev/null; then
    echo "error: ci.sh requires nightly Rust, rustfmt, clippy, and cargo-expand"
    exit 1
fi

echo "Running 'cargo ${toolchain} fmt --all'"
cargo "${toolchain}" fmt --all

echo "Running 'cargo ${toolchain} clippy --all --all-targets'"
cargo "${toolchain}" clippy --all --all-features --all-targets -Zunstable-options

echo "Running 'cargo ${toolchain} test --all --exclude expandtest'"
TRYBUILD=overwrite cargo "${toolchain}" test --all --all-features --exclude expandtest

echo "Running 'bash scripts/expandtest.sh ${toolchain}'"
"$(cd "$(dirname "${0}")" && pwd)"/expandtest.sh "${toolchain}"

echo "Running 'cargo ${toolchain} doc --no-deps --all'"
cargo "${toolchain}" doc --no-deps --all --all-features
