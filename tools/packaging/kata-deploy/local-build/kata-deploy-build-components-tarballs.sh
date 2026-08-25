#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${script_dir}/../../../.." && pwd)"

# shellcheck source=tools/packaging/scripts/lib.sh
source "${script_dir}/../../scripts/lib.sh"
build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
component="${1:-all}"
versions_yaml="${repo_root_dir}/versions.yaml"

mkdir -p "${build_dir}"

get_rust_toolchain_version() {
	awk '
		/^languages:/ { in_languages=1; next }
		in_languages && /^[^[:space:]]/ { in_languages=0 }
		in_languages && /^  rust:/ { in_rust=1; next }
		in_rust && /^  [^[:space:]]/ { in_rust=0 }
		in_rust && /^    version:/ {
			gsub(/"/, "", $2)
			print $2
			exit
		}
	' "${versions_yaml}"
}

rust_toolchain="$(get_rust_toolchain_version)"
if [[ -z "${rust_toolchain}" ]]; then
	echo "Failed to extract languages.rust.version from ${versions_yaml}" >&2
	exit 1
fi

rust_builder_tar="${build_dir}/kata-deploy-binary-out.tar"

build_kata_deploy_binary() {
	rm -f "${rust_builder_tar}"
	docker buildx build \
		--target rust-builder \
		--build-arg "RUST_TOOLCHAIN=${rust_toolchain}" \
		--output "type=tar,dest=${rust_builder_tar}" \
		-f "${repo_root_dir}/tools/packaging/kata-deploy/Dockerfile.components" \
		"${repo_root_dir}"

	mkdir -p "${build_dir}/kata-deploy-binary/usr/bin"
	tar -xf "${rust_builder_tar}" -C "${build_dir}/kata-deploy-binary/usr/bin" \
		--strip-components=2 kata-deploy/bin/kata-deploy
	kata_tar_zstd -cf "${build_dir}/kata-deploy-static-kata-deploy-binary.tar.zst" \
		-C "${build_dir}/kata-deploy-binary" .
}

build_nydus_snapshotter_for_coco_guest_pull() {
	# The kata Go host binaries are built static by default (see
	# static-build/shim-v2). STATIC_RUNTIME=yes additionally pulls the
	# statically linked nydus-snapshotter binaries instead of the glibc-linked
	# per-arch ones, producing a fully musl-compatible payload. The static
	# nydus asset is published for amd64 only.
	local static_nydus_snapshotter="${STATIC_RUNTIME:-}"

	docker buildx build \
		--target nydus-binary-downloader \
		--build-arg "STATIC_NYDUS_SNAPSHOTTER=${static_nydus_snapshotter}" \
		--output "type=local,dest=${build_dir}/nydus-snapshotter-out" \
		-f "${repo_root_dir}/tools/packaging/kata-deploy/Dockerfile.components" \
		"${repo_root_dir}"

	mkdir -p "${build_dir}/nydus-snapshotter/opt/kata-artifacts/nydus-snapshotter"
	cp "${build_dir}/nydus-snapshotter-out/opt/nydus-snapshotter/bin/containerd-nydus-grpc" \
		"${build_dir}/nydus-snapshotter/opt/kata-artifacts/nydus-snapshotter/"
	cp "${build_dir}/nydus-snapshotter-out/opt/nydus-snapshotter/bin/nydus-overlayfs" \
		"${build_dir}/nydus-snapshotter/opt/kata-artifacts/nydus-snapshotter/"
	kata_tar_zstd -cf "${build_dir}/kata-deploy-static-nydus-snapshotter-for-coco-guest-pull.tar.zst" \
		-C "${build_dir}/nydus-snapshotter" .
}

case "${component}" in
	kata-deploy-binary) build_kata_deploy_binary ;;
	nydus-snapshotter-for-coco-guest-pull) build_nydus_snapshotter_for_coco_guest_pull ;;
	all)
		build_kata_deploy_binary
		build_nydus_snapshotter_for_coco_guest_pull
		;;
	*)
		echo "Unknown component '${component}'. Expected: kata-deploy-binary, nydus-snapshotter-for-coco-guest-pull, all" >&2
		exit 1
		;;
esac
