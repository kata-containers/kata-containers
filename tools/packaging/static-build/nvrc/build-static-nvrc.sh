#!/usr/bin/env bash
#
# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

# shellcheck disable=SC2154
[[ -n "${nvrc_repo}" ]] || die "failed to get nvrc repo"
# shellcheck disable=SC2154
[[ -n "${nvrc_ref}" ]] || die "failed to get nvrc git ref"
# shellcheck disable=SC2154
[[ -n "${nvrc_toolchain}" ]] || die "failed to get nvrc rust toolchain"

# The NVRC checkout lives inside the kata-containers tree, whose root carries a
# rust-toolchain.toml. rustup walks parent directories and would honour that pin
# (a different channel than the one baked into this builder image, which is the
# only toolchain with the musl target installed). Force our toolchain so the
# build is decoupled from kata's root pin and the musl std is always found.
export RUSTUP_TOOLCHAIN="${nvrc_toolchain}"

[[ -d "nvrc" ]] && rm -rf nvrc

build_nvrc_from_source() {
	echo "build nvrc from source"

	# shellcheck source=/dev/null
	. /etc/profile.d/rust.sh

	# `ref` may be a branch, tag or commit, so clone then fetch the exact ref.
	git clone "${nvrc_repo}" nvrc
	pushd nvrc

	git fetch origin "${nvrc_ref}"
	git checkout FETCH_HEAD

	# shellcheck disable=SC2154
	local target="${RUST_ARCH}-unknown-linux-musl"
	cargo build --release --target "${target}"
	strip "target/${target}/release/NVRC"

	# Mirror the NVRC release tarball layout: the init binary lands in /bin
	# named for its target triple. nvidia_rootfs.sh extracts this tarball
	# verbatim into the guest rootfs.
	local nvrc_name="NVRC-${target}"
	install -D -m0755 "target/${target}/release/NVRC" "${DESTDIR}/bin/${nvrc_name}"

	popd
}

build_nvrc_from_source "$@"
