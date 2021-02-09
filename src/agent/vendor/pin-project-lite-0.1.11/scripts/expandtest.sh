#!/bin/bash

# A script to run expandtest.
#
# Usage:
#     bash scripts/expandtest.sh
#
# Note: This script requires nightly Rust, rustfmt, and cargo-expand

set -euo pipefail

script_dir="$(cd "$(dirname "${0}")" && pwd)"

if [[ "${1:-none}" == "+"* ]]; then
    toolchain="${1}"
elif [[ "${CI:-false}" != "true" ]]; then
    cargo +nightly -V >/dev/null || exit 1
    toolchain="+nightly"
fi

if [[ "${toolchain:-+nightly}" != "+nightly"* ]] || ! rustfmt -V &>/dev/null || ! cargo expand -V &>/dev/null; then
    echo "error: expandtest.sh requires nightly Rust, rustfmt, and cargo-expand"
    exit 1
fi

if [[ "${CI:-false}" != "true" ]]; then
    # First, check if the compile fails for another reason.
    cargo ${toolchain} check --tests --manifest-path "${script_dir}"/../tests/expand/Cargo.toml

    # Next, remove the `*.expanded.rs` files to allow updating those files.
    # Refs: https://docs.rs/macrotest/1/macrotest/#updating-expandedrs
    rm -rf "${script_dir}"/../tests/expand/tests/expand/*.expanded.rs
fi

cargo ${toolchain:-} test --manifest-path "${script_dir}"/../tests/expand/Cargo.toml
