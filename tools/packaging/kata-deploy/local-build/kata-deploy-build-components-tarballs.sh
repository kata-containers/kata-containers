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

rust_builder_out="${build_dir}/kata-deploy-binary-out"

# kata-deploy and kata-deploy-job-dispatcher are produced by the same rust-builder
# stage. Build it once *per process* and let each component package its own
# binary, so running both components in a single invocation does not compile the
# workspace twice. The guard is process-local (not a directory check) on purpose:
# a fresh invocation must always rebuild, otherwise a stale output dir from an
# earlier run/commit would be silently reused.
rust_binaries_built="false"
build_rust_binaries() {
	if [[ "${rust_binaries_built}" == "true" ]]; then
		return
	fi
	rm -rf "${rust_builder_out}"
	docker buildx build \
		--target rust-builder \
		--build-arg "RUST_TOOLCHAIN=${rust_toolchain}" \
		--output "type=local,dest=${rust_builder_out}" \
		-f "${repo_root_dir}/tools/packaging/kata-deploy/Dockerfile.components" \
		"${repo_root_dir}"
	rust_binaries_built="true"
}

build_kata_deploy_binary() {
	build_rust_binaries

	mkdir -p "${build_dir}/kata-deploy-binary/usr/bin"
	cp "${rust_builder_out}/kata-deploy/bin/kata-deploy" \
		"${build_dir}/kata-deploy-binary/usr/bin/kata-deploy"
	tar --zstd -cf "${build_dir}/kata-deploy-static-kata-deploy-binary.tar.zst" \
		-C "${build_dir}/kata-deploy-binary" .
}

build_kata_deploy_job_dispatcher() {
	build_rust_binaries

	mkdir -p "${build_dir}/kata-deploy-job-dispatcher/usr/bin"
	cp "${rust_builder_out}/kata-deploy/bin/kata-deploy-job-dispatcher" \
		"${build_dir}/kata-deploy-job-dispatcher/usr/bin/kata-deploy-job-dispatcher"
	tar --zstd -cf "${build_dir}/kata-deploy-static-kata-deploy-job-dispatcher.tar.zst" \
		-C "${build_dir}/kata-deploy-job-dispatcher" .
}

build_nydus_snapshotter_for_coco_guest_pull() {
	docker buildx build \
		--target nydus-binary-downloader \
		--output "type=local,dest=${build_dir}/nydus-snapshotter-out" \
		-f "${repo_root_dir}/tools/packaging/kata-deploy/Dockerfile.components" \
		"${repo_root_dir}"

	mkdir -p "${build_dir}/nydus-snapshotter/opt/kata-artifacts/nydus-snapshotter"
	cp "${build_dir}/nydus-snapshotter-out/opt/nydus-snapshotter/bin/containerd-nydus-grpc" \
		"${build_dir}/nydus-snapshotter/opt/kata-artifacts/nydus-snapshotter/"
	cp "${build_dir}/nydus-snapshotter-out/opt/nydus-snapshotter/bin/nydus-overlayfs" \
		"${build_dir}/nydus-snapshotter/opt/kata-artifacts/nydus-snapshotter/"
	tar --zstd -cf "${build_dir}/kata-deploy-static-nydus-snapshotter-for-coco-guest-pull.tar.zst" \
		-C "${build_dir}/nydus-snapshotter" .
}

case "${component}" in
	kata-deploy-binary) build_kata_deploy_binary ;;
	kata-deploy-job-dispatcher) build_kata_deploy_job_dispatcher ;;
	nydus-snapshotter-for-coco-guest-pull) build_nydus_snapshotter_for_coco_guest_pull ;;
	all)
		build_kata_deploy_binary
		build_kata_deploy_job_dispatcher
		build_nydus_snapshotter_for_coco_guest_pull
		;;
	*)
		echo "Unknown component '${component}'. Expected: kata-deploy-binary, kata-deploy-job-dispatcher, nydus-snapshotter-for-coco-guest-pull, all" >&2
		exit 1
		;;
esac
