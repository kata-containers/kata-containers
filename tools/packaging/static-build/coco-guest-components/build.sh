#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly coco_guest_components_builder="${script_dir}/build-static-coco-guest-components.sh"

source "${script_dir}/../../scripts/lib.sh"

DESTDIR=${DESTDIR:-${PWD}}

coco_guest_components_repo="${coco_guest_components_repo:-}"
coco_guest_components_version="${coco_guest_components_version:-}"
coco_guest_components_toolchain="${coco_guest_components_toolchain:-}"
package_output_dir="${package_output_dir:-}"

[ -n "${coco_guest_components_repo}" ] || coco_guest_components_repo=$(get_from_kata_deps ".externals.coco-guest-components.url")
[ -n "${coco_guest_components_version}" ] || coco_guest_components_version=$(get_from_kata_deps ".externals.coco-guest-components.version")
[ -n "${coco_guest_components_toolchain}" ] || coco_guest_components_toolchain=$(get_from_kata_deps ".externals.coco-guest-components.toolchain")

[ -n "${coco_guest_components_repo}" ] || die "Failed to get coco-guest-components repo"
[ -n "${coco_guest_components_version}" ] || die "Failed to get coco-guest-components version or commit"
[ -n "${coco_guest_components_toolchain}" ] || die "Failed to get the rust toolchain to build coco-guest-components"

container_image="${COCO_GUEST_COMPONENTS_CONTAINER_BUILDER:-$(get_coco_guest_components_image_name)}"
[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image}-cross-build"

docker pull ${container_image} || \
	(docker $BUILDX build $PLATFORM \
	    	--build-arg RUST_TOOLCHAIN="${coco_guest_components_toolchain}" \
		-t "${container_image}" "${script_dir}" && \
	 # No-op unless PUSH_TO_REGISTRY is exported as "yes"
	 push_to_registry "${container_image}")

# Temp settings until we have a matching TEE_PLATFORM
TEE_PLATFORM=""
RESOURCE_PROVIDER="kbs,sev"
# snp-attester and tdx-attester crates require packages only available on x86
# se-attester crate requires packages only available on s390x
case "$(uname -m)" in
	x86_64) ATTESTER="snp-attester,tdx-attester" ;;
	s390x) ATTESTER="se-attester" ;;
	*) ATTESTER="none" ;;
esac

docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" \
	--env TEE_PLATFORM=${TEE_PLATFORM:+"all"} \
	--env RESOURCE_PROVIDER=${RESOURCE_PROVIDER:-} \
	--env ATTESTER=${ATTESTER:-} \
	--env coco_guest_components_repo="${coco_guest_components_repo}" \
	--env coco_guest_components_version="${coco_guest_components_version}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${coco_guest_components_builder}"
