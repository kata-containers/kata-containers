#!/bin/bash

# A script to run a simplified version of the checks done by CI.
#
# Usage:
#     bash scripts/ci.sh
#
# Note: This script requires nightly Rust, rustfmt, clippy, and cargo-expand

set -euo pipefail
IFS=$'\n\t'

# Decide Rust toolchain. Nightly is used by default.
toolchain="+nightly"
if [[ "${1:-}" == "+"* ]]; then
  toolchain="${1}"
  shift
fi
# Make sure toolchain is installed.
cargo "${toolchain}" -V >/dev/null

if [[ "${toolchain:-+nightly}" != "+nightly"* ]] || ! rustfmt -V &>/dev/null || ! cargo clippy -V &>/dev/null || ! cargo expand -V &>/dev/null; then
  echo "error: ci.sh requires nightly Rust, rustfmt, clippy, and cargo-expand"
  exit 1
fi

echo "Running 'cargo ${toolchain} fmt --all'"
cargo "${toolchain}" fmt --all

echo "Running 'cargo ${toolchain} clippy --all --all-targets'"
cargo "${toolchain}" clippy --all --all-features --all-targets -Z unstable-options

echo "Running 'cargo ${toolchain} test --all'"
cargo "${toolchain}" test --all --all-features

echo "Running 'cargo ${toolchain} doc --no-deps --all'"
cargo "${toolchain}" doc --no-deps --all --all-features
