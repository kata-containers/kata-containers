#!/bin/bash

set -euo pipefail

toolchain="${1:-nightly}"

rustup toolchain install "${toolchain}" --no-self-update --profile minimal
rustup default "${toolchain}"

rustup -V
rustc -V
cargo -V
