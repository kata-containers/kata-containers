#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=/dev/null
source "${script_dir}/../scripts/lib.sh"

agent_tarball=${AGENT_TARBALL:?AGENT_TARBALL is required}
destination=${DESTDIR:?DESTDIR is required}/opt/kata/share/kata-containers
arch=$(uname -m)
case "${arch}" in
	x86_64) platform=linux/amd64 ;;
	aarch64) platform=linux/arm64 ;;
	*) die "Unsupported Substrate architecture: ${arch}" ;;
esac
base_image=${SUBSTRATE_BASE_IMAGE:-$(get_from_kata_deps \
	".assets.image.architecture.${arch}.substrate | .name + \":\" + .version")}

context=$(mktemp -d)
trap 'rm -rf "${context}"' EXIT
# shellcheck disable=SC2154
osbuilder="${repo_root_dir}/tools/osbuilder"
cp "${osbuilder}/rootfs-builder/debian/Dockerfile.substrate" "${context}/Dockerfile"
cp "${osbuilder}/rootfs-builder/debian/substrate-prune.sh" "${context}/prune.sh"
mkdir -p "${context}/osbuilder/image-builder/tests" "${context}/osbuilder/scripts"
cp "${osbuilder}/image-builder/image_builder.sh" "${context}/osbuilder/image-builder/"
cp "${osbuilder}/image-builder/tests/substrate.sh" "${context}/osbuilder/image-builder/tests/"
cp "${osbuilder}/scripts/lib.sh" "${context}/osbuilder/scripts/"
tar --zstd -xf "${agent_tarball}" -C "${context}"
install -m 0755 "${context}/usr/bin/kata-agent" "${context}/kata-agent"
docker buildx build --pull --platform "${platform}" \
	--build-arg DEBIAN_IMAGE="${base_image}" \
	--target export --output "type=local,dest=${context}/out" "${context}"

install -D -m 0644 "${context}/out/rootfs.img" "${destination}/kata-containers-substrate.img"
install -m 0644 "${context}/out/rootfs-packages.txt" "${destination}/kata-containers-substrate-packages.txt"
install -m 0644 "${context}/out/image-packages.txt" "${destination}/kata-containers-substrate-builder-packages.txt"
