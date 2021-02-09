#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

toolchain="${1:-nightly}"

# --no-self-update is necessary because the windows environment cannot self-update rustup.exe.
rustup toolchain install "${toolchain}" --no-self-update --profile minimal
rustup default "${toolchain}"
