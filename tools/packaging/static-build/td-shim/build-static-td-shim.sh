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

# Cargo resolves the whole workspace: tests/test-td-payload depends on ring
# unconditionally and [patch.crates-io] points into the submodule, so it must
# be checked out even though the secure-boot feature (ring's only user) stays
# disabled below. The patch set in sh_script/preparation.sh is only needed
# when ring is actually compiled, so it is not applied.
git submodule update --init --recursive

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

payload_layout_json="$(jq --arg size "${payload_region_size}" '
	(.memory_regions[] | select(.name == "Payload") | .size) = $size
' "${payload_layout_config}")"
printf '%s\n' "${payload_layout_json}" > "${payload_layout_config}"

cargo run -p td-layout-config -- \
	-t memory "${payload_layout_config}" \
	-o "${payload_layout_file}"

grep -q "pub const PAYLOAD_SIZE: usize = ${payload_region_size};" "${payload_layout_file}" ||
	die "failed to set the td-shim payload region to 64MB in ${payload_layout_file}"

# dragonball boots the kernel through the TD HOB, so td-shim's secure-boot
# payload verification never runs. Build without default features to drop
# secure-boot (and the vendored ring crate it pulls in); same for td-shim-ld,
# whose default features enable the ring-based enroller/signer tools.
cargo build -p td-shim --target x86_64-unknown-none --release \
	--no-default-features --features=main,tdx
cargo run -p td-shim-tools --bin td-shim-ld \
	--no-default-features --features=linker -- \
	target/x86_64-unknown-none/release/ResetVector.bin \
	target/x86_64-unknown-none/release/td-shim \
	-o target/release/final.bin

final_bin="target/release/final.bin"
[[ -f "${final_bin}" ]] || die "td-shim binary not found at ${final_bin}"

info "install td-shim to ${DESTDIR}${PREFIX}/share/td-shim/td-shim.bin"
install -D --mode 0644 "${final_bin}" "${DESTDIR}${PREFIX}/share/td-shim/td-shim.bin"
popd # td-shim
popd # ${build_root}
