#!/usr/bin/env bash
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

ARCH=${ARCH:-$(uname -m)}

# OpenVMM currently only supports x86_64 and aarch64.
[[ "${ARCH}" != "aarch64" ]] && [[ "${ARCH}" != "x86_64" ]] && exit

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

openvmm_repo="${openvmm_repo:-}"
openvmm_version="${openvmm_version:-}"

[[ -n "${openvmm_repo}" ]] || openvmm_repo=$(get_from_kata_deps ".assets.hypervisor.openvmm.url")
[[ -n "${openvmm_version}" ]] || openvmm_version=$(get_from_kata_deps ".assets.hypervisor.openvmm.version")

[[ -n "${openvmm_repo}" ]] || die "failed to get openvmm repo"
[[ -n "${openvmm_version}" ]] || die "failed to get openvmm version"

# Unlike cloud-hypervisor, OpenVMM does not publish pre-built release binaries,
# so it is always built from source at the pinned commit.
build_openvmm_from_source() {
	info "build openvmm from source: ${openvmm_repo} @ ${openvmm_version}"

	rm -rf openvmm-src openvmm
	git clone "${openvmm_repo}" openvmm-src
	pushd openvmm-src
	git checkout "${openvmm_version}"

	# OpenVMM has no rust-toolchain.toml and requires a newer Rust than the Kata
	# runtime toolchain, so pin explicitly to its declared MSRV (Cargo.toml
	# rust-version) instead of relying on the default/stable channel.
	# restore-packages fetches build inputs (e.g. protoc) the build needs.
	local rust_version
	rust_version=$(grep -m1 -E '^[[:space:]]*rust-version[[:space:]]*=' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
	[[ -n "${rust_version}" ]] || die "failed to read openvmm rust-version from Cargo.toml"
	info "building openvmm with Rust ${rust_version}"
	rustup toolchain install "${rust_version}" --profile minimal

	# --no-compat-igvm skips downloading released OpenHCL IGVM files (used only
	# for VMM compatibility testing, not for building the openvmm binary). Those
	# come from GitHub releases and would trigger an interactive `gh auth login`
	# device-code prompt, hanging the non-interactive build. OpenVMM's own CI
	# (.github/copilot-setup-steps.yml) uses the same flag for this reason.
	cargo "+${rust_version}" xflowey restore-packages --no-compat-igvm
	cargo "+${rust_version}" build --release --package openvmm

	local binary="target/release/openvmm"
	[[ -f "${binary}" ]] || die "openvmm binary not found at ${binary}"
	# OpenVMM release builds include DWARF; remove it so the combined Kata
	# release tarball remains below GitHub's 2 GiB asset limit.
	strip --strip-debug "${binary}"
	popd

	# Stage the binary at openvmm/openvmm for the installer to pick up
	# (a dedicated directory avoids clashing with the openvmm/ source tree).
	mkdir -p openvmm
	cp -f "openvmm-src/${binary}" openvmm/openvmm
	chmod +x openvmm/openvmm

	# build.sh runs this container as root (so restore-packages can apt-install
	# its build inputs); hand the generated files back to the invoking user so
	# the unprivileged outer build/packaging steps can read and clean them.
	if [[ -n "${HOST_UID:-}" ]]; then
		chown -R "${HOST_UID}:${HOST_GID:-${HOST_UID}}" openvmm openvmm-src
	fi
}

build_openvmm_from_source
