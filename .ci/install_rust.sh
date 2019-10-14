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
# recent functional version
version="2019-10-04"

if ! command -v rustup > /dev/null; then
	curl https://sh.rustup.rs -sSf | sh
fi

rustup toolchain install ${release}-${version}-${rustarch}-unknown-linux-gnu
rustup default ${release}-${version}-${rustarch}-unknown-linux-gnu
rustup target install ${rustarch}-unknown-linux-musl
rustup component add rustfmt
sudo ln -sf /usr/bin/g++ /bin/musl-g++
