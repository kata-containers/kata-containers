#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for the split between the chart defaults and the
# try-*.values.yaml profiles. No cluster required.
#
# values.yaml enables every shim it lists, so what it lists decides what a plain
# `helm install` promises: a shim needing a snapshotter nobody set up, or hardware
# the node does not have, becomes a RuntimeClass that looks available and fails the
# first pod scheduled onto it. Those shims live in the profiles instead, which is a
# property of a values file rather than of any template - nothing else here would
# notice it being undone.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

setup_file() {
	ensure_yq
}

render() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		"$@"
}

# The RuntimeClass names a set of values renders, one per line.
runtime_classes() {
	render --show-only templates/runtimeclasses.yaml "$@" |
		awk '/^  name: /{print $2}' | sort
}

# The value of one env var. Every container that gets it gets the same value, so
# one is enough - and the containers that do not have it must not turn an empty
# answer into a non-empty one.
env_value() {
	local rendered="${1}" name="${2}"
	echo "${rendered}" | grep -A1 "name: ${name}$" | grep 'value:' |
		sed -e 's/.*value: //' -e 's/^"//' -e 's/"$//' | sort -u | head -1
}

# The shims a profile turns on, read from the values file itself rather than from
# what it rendered: that is what makes a shim silently dropped by the render
# (no supportedArches, say) a failure rather than an empty comparison.
enabled_shims() {
	yq -r '.shims | to_entries | map(select(.value.enabled == true) | .key) | .[]' "${1}" | sort
}

@test "Helm template: the default install needs no snapshotter" {
	# Nothing to set up, and no shim asking for one: the default set reads images
	# through whatever snapshotter containerd already uses.
	local rendered arch
	rendered=$(render)

	[[ -z "$(env_value "${rendered}" EXPERIMENTAL_SETUP_SNAPSHOTTER)" ]]
	for arch in X86_64 AARCH64 S390X PPC64LE; do
		[[ -z "$(env_value "${rendered}" "SNAPSHOTTER_HANDLER_MAPPING_${arch}")" ]]
	done
}

@test "Helm template: the default install creates only generic RuntimeClasses" {
	# Confidential, NVIDIA, Firecracker and peer-pod classes each need something
	# the chart cannot arrange on its own, so they come from a profile.
	local class
	while read -r class; do
		if [[ "${class}" =~ (snp|tdx|-se$|coco|nvidia|^kata-fc|^kata-remote) ]]; then
			echo "the chart defaults render ${class}, which needs a snapshotter" \
				"or hardware a default install cannot promise" >&2
			return 1
		fi
	done < <(runtime_classes)
}

@test "Helm template: enabling a shim the defaults do not carry is refused" {
	# Without supportedArches the shim is installed on no architecture and still
	# gets a RuntimeClass, so the install looks fine until the first pod hangs in
	# ContainerCreating. The chart says so instead, and names the way out.
	run render --set shims.disableAll=true --set shims.qemu-tdx.enabled=true
	[ "${status}" -ne 0 ]
	[[ "${output}" =~ "qemu-tdx" ]]
	[[ "${output}" =~ "supportedArches is empty" ]]
	[[ "${output}" =~ "try-kata-" ]]
}

@test "Helm template: the profiles are how the tests find a shim's definition" {
	# deploy_kata installs the profile defining the shim under test, found by
	# looking the shim up in each profile. A shim no profile claims would leave it
	# describing the shim itself, which is how it once came to install nothing.
	local profile shim
	for profile in "${CHART_PATH}"/try-kata-*.values.yaml; do
		while read -r shim; do
			[[ "$(shim_profile_file "${shim}")" == "${profile}" ]] || {
				echo "shim_profile_file ${shim} did not find ${profile}" >&2
				return 1
			}
		done < <(enabled_shims "${profile}")
	done
}

@test "Helm template: every profile renders the shims it enables" {
	# A shim block carrying no supportedArches matches no architecture, which the
	# chart now refuses outright - and refusing it is what makes this comparison
	# fail rather than come out empty on both sides.
	local profile shim classes
	for profile in "${CHART_PATH}"/try-kata-*.values.yaml; do
		classes=$(runtime_classes -f "${profile}")
		[[ -n "${classes}" ]] || {
			echo "${profile} renders no RuntimeClass at all" >&2
			return 1
		}

		while read -r shim; do
			echo "${classes}" | grep -qx "kata-${shim}" || {
				echo "${profile} enables ${shim} but renders no kata-${shim}" >&2
				return 1
			}
		done < <(enabled_shims "${profile}")
	done
}

@test "Helm template: a profile sets up every snapshotter its shims are mapped to" {
	# A shim mapped to a snapshotter that was never set up fails at pod start, and
	# a profile is the one place that can get this right for the shims it ships.
	# devmapper is the exception: kata-deploy cannot set it up (it needs a host
	# thin-pool), which is why try-kata-fc.values.yaml documents it as a
	# prerequisite instead.
	local profile rendered setup arch snapshotter mapping
	for profile in "${CHART_PATH}"/try-kata-*.values.yaml; do
		rendered=$(render -f "${profile}")
		setup=$(env_value "${rendered}" EXPERIMENTAL_SETUP_SNAPSHOTTER)

		for arch in X86_64 AARCH64 S390X PPC64LE; do
			mapping=$(env_value "${rendered}" "SNAPSHOTTER_HANDLER_MAPPING_${arch}")
			[[ -n "${mapping}" ]] || continue

			while read -r snapshotter; do
				[[ "${snapshotter}" == "devmapper" ]] && continue
				echo "${setup}" | tr ',' '\n' | grep -qx "${snapshotter}" || {
					echo "${profile} maps a shim to ${snapshotter} on ${arch}" \
						"but does not set it up (setup: '${setup}')" >&2
					return 1
				}
			done < <(echo "${mapping}" | tr ',' '\n' | cut -d: -f2 | sort -u)
		done
	done
}
