#!/usr/bin/env bash
#
# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

ARCH=${ARCH:-$(uname -m)}
[[ "${ARCH}" != "x86_64" ]] && exit 0

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}

tdshim_repo="${tdshim_repo:-}"
tdshim_version="${tdshim_version:-}"

[[ -n "${tdshim_repo}" ]] || die "failed to get td-shim repo"
[[ -n "${tdshim_version}" ]] || die "failed to get td-shim version"

info "build td-shim from source: ${tdshim_repo} @ ${tdshim_version}"

build_root=$(mktemp -d)
pushd "${build_root}"
git clone "${tdshim_repo}" td-shim
pushd td-shim
git checkout "${tdshim_version}"

# The vendored ring crate ships as a submodule and needs the upstream patch
# set applied on top of its pinned commit before building.
git submodule update --init --recursive
./sh_script/preparation.sh

# td-shim's runtime layout for the Linux payload (td-layout/src/runtime/linux.rs)
# reserves only 32MB for the payload region at 0x861000. Kata does not embed
# the kernel into the firmware: dragonball loads the kernel ELF at its own
# physical address and passes the entry point through the TD HOB, so a ~40MB
# kernel image extends past the 32MB reservation. Grow the region to 64MB by
# updating devtools/td-layout-config/config_memory_linux.json and regenerating
# the layout with the upstream td-layout-config tool. td-shim-ld derives the
# TDVF Payload metadata from the same constants, so the regenerated layout
# covers both the firmware and the metadata.
payload_layout_config="devtools/td-layout-config/config_memory_linux.json"
payload_layout_file="td-layout/src/runtime/linux.rs"
payload_region_size="0x4000000"

# The size of a region is on the line right after its name.
sed -i "/\"name\": \"Payload\",/{n;s/\"size\": \"[^\"]*\"/\"size\": \"${payload_region_size}\"/}" "${payload_layout_config}"

cargo run -p td-layout-config -- \
	-t memory "${payload_layout_config}" \
	-o "${payload_layout_file}"

grep -q "pub const PAYLOAD_SIZE: usize = ${payload_region_size};" "${payload_layout_file}" ||
	die "failed to set the td-shim payload region to 64MB in ${payload_layout_file}"

# Per the td-shim README: clang/llvm-ar build the C/asm parts (vendored ring).
export CC=clang
export AR=llvm-ar
export CC_x86_64_unknown_none=clang
export AR_x86_64_unknown_none=llvm-ar

cargo build -p td-shim --target x86_64-unknown-none --release --features=main,tdx
cargo run -p td-shim-tools --bin td-shim-ld --features=linker -- \
	target/x86_64-unknown-none/release/ResetVector.bin \
	target/x86_64-unknown-none/release/td-shim \
	-o target/release/final.bin

final_bin="target/release/final.bin"
[[ -f "${final_bin}" ]] || die "td-shim binary not found at ${final_bin}"

info "install td-shim to ${DESTDIR}${PREFIX}/share/td-shim/td-shim.bin"
install -D --mode 0644 "${final_bin}" "${DESTDIR}${PREFIX}/share/td-shim/td-shim.bin"
popd # td-shim
popd # ${build_root}
