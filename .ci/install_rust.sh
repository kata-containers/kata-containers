#!/bin/bash
#
# Copyright (c) 2019 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0

set -e

[ -n "${KATA_DEV_MODE:-}" ] && exit 0

cidir=$(dirname "$0")
rustarch=$(${cidir}/kata-arch.sh --rust)
release="nightly"

if ! command -v rustup > /dev/null; then
	curl https://sh.rustup.rs -sSf | sh
fi

rustup toolchain install ${release}-${rustarch}-unknown-linux-gnu
rustup default ${release}-${rustarch}-unknown-linux-gnu
rustup target install ${rustarch}-unknown-linux-musl
ln -sf /usr/bin/g++ /bin/musl-g++
