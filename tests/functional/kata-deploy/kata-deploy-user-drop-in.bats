#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# The containerd user drop-in has to belong to the installation that wrote it.
# One fixed file name meant two installations sharing it, and either uninstall
# taking it from the other - on a host with no fs-verity, that is enough to stop
# containerd loading the CRI plugin and take the node NotReady.
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, clh, etc.)
#   KUBERNETES      - K8s distribution (microk8s, k3s, rke2, etc.)

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

DROP_IN_SUFFIX="beta"
SUFFIXED_DROP_IN="zz-kata-deploy-user-${DROP_IN_SUFFIX}.toml"
# The name every unsuffixed installation writes, and the one this suffixed
# installation used to write too.
SHARED_DROP_IN="zz-kata-deploy-user.toml"

DROP_IN_ROOTS="$(containerd_config_roots) /host/opt/kata-${DROP_IN_SUFFIX}"

# Where ${1} was written, or NONE. The directory is the distribution's to pick,
# so find the file rather than predict the path.
drop_in_paths() {
	run_on_host "find ${DROP_IN_ROOTS} -name ${1} 2>/dev/null; echo NONE"
}

setup_file() {
	ensure_helm

	echo "# Image: ${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" >&3
	echo "# Hypervisor: ${KATA_HYPERVISOR}" >&3
	echo "# K8s distribution: ${KUBERNETES}" >&3

	# Nothing containerd does not already default to: this suite is about which
	# file the setting lands in, not the setting.
	USER_DROP_IN_VALUES="$(mktemp)"
	cat > "${USER_DROP_IN_VALUES}" <<-EOF
		containerd:
		  userDropIn: |
		    [debug]
		      format = "text"
	EOF
	export USER_DROP_IN_VALUES

	echo "# Deploying kata-deploy with env.multiInstallSuffix=${DROP_IN_SUFFIX}..." >&3
	deploy_kata "${USER_DROP_IN_VALUES}" \
		--set "env.multiInstallSuffix=${DROP_IN_SUFFIX}"
	echo "# kata-deploy deployed successfully" >&3
}

@test "A suffixed installation writes a user drop-in of its own" {
	run drop_in_paths "${SUFFIXED_DROP_IN}"
	echo "# ${SUFFIXED_DROP_IN}: ${output}" >&3
	[[ "${output}" == *"/host/"* ]]

	# Without this, finding the file gone later would say nothing: an empty one
	# that never worked would satisfy it just as well.
	local drop_in
	drop_in="$(echo "${output}" | grep -m1 '^/host/')"
	run run_on_host "grep -q 'format' ${drop_in} && echo CARRIED || echo EMPTY"
	echo "# ${drop_in}: ${output}" >&3
	[[ "${output}" == *"CARRIED"* ]]

	# That name is the next installation's to own.
	run drop_in_paths "${SHARED_DROP_IN}"
	echo "# ${SHARED_DROP_IN}: ${output}" >&3
	[[ "${output}" == "NONE" ]]
}

@test "Uninstalling it leaves another installation's user drop-in alone" {
	run drop_in_paths "${SUFFIXED_DROP_IN}"
	# Unfound, dirname would give ".", and every check below would pass there.
	[[ "${output}" == *"/host/"* ]]
	local drop_in_dir
	drop_in_dir="$(dirname "$(echo "${output}" | grep -m1 '^/host/')")"

	# microk8s and friends keep the drop-in under the installation prefix, which
	# uninstall removes whole. Two installations cannot land in one directory
	# there, so the file name they used to collide over is not theirs to share.
	if [[ "${drop_in_dir}" == "/host/opt/kata-${DROP_IN_SUFFIX}/"* ]]; then
		skip "each installation owns its drop-in directory here"
	fi

	# Stands in for an installation alongside this one: deploying a second
	# release just to produce the file would test helm, not this uninstall.
	# A comment, so containerd loads it whatever its config schema.
	run_on_host "echo '# another installation owns this file' > ${drop_in_dir}/${SHARED_DROP_IN}" false

	echo "# Uninstalling the ${DROP_IN_SUFFIX} installation..." >&3
	uninstall_kata

	run run_on_host "test -f ${drop_in_dir}/${SHARED_DROP_IN} && echo KEPT || echo REMOVED"
	echo "# ${SHARED_DROP_IN}: ${output}" >&3
	[[ "${output}" == *"KEPT"* ]]

	# Its own, on the other hand, is its own to remove.
	run drop_in_paths "${SUFFIXED_DROP_IN}"
	echo "# ${SUFFIXED_DROP_IN}: ${output}" >&3
	[[ "${output}" == "NONE" ]]

	# A containerd that can no longer load its CRI plugin shows up here.
	kubectl wait nodes --timeout=300s --all --for condition=Ready=True
}

teardown() {
	if [[ "${BATS_TEST_COMPLETED:-}" != "1" && -z "${BATS_TEST_SKIPPED:-}" ]]; then
		kubectl -n "${HELM_NAMESPACE}" logs -l "$(kata_deploy_pod_selector)" 2>/dev/null || true
	fi
}

teardown_file() {
	# The stand-in belongs to no installation, so nothing else will remove it.
	run_on_host "find ${DROP_IN_ROOTS} -name ${SHARED_DROP_IN} -delete" false \
		2>/dev/null || true
	rm -f "${USER_DROP_IN_VALUES:-}" 2>/dev/null || true
	uninstall_kata 2>/dev/null || true
}
