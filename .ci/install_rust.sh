#!/bin/bash
#
# Copyright (c) 2019 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0

set -e

[ -n "${KATA_DEV_MODE:-}" ] && exit 0

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

rustarch=$(${cidir}/kata-arch.sh --rust)
# release="nightly"
# recent functional version
version="${1:-""}"
if [ -z "${version}" ]; then
	version=$(get_version "languages.rust.meta.newest-version")
fi

if ! command -v rustup > /dev/null; then
	curl https://sh.rustup.rs -sSf | sh
fi

export PATH="${PATH}:${HOME}/.cargo/bin"

rustup toolchain install ${version}
rustup default ${version}
rustup target install ${rustarch}-unknown-linux-musl
rustup component add rustfmt
sudo ln -sf /usr/bin/g++ /bin/musl-g++
