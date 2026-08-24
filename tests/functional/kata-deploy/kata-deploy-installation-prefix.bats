#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Kata Deploy Installation Prefix Tests
#
# Installing anywhere other than /opt/kata makes kata-deploy write a drop-in
# that moves the paths of every shim it configures, so a shim the drop-in gets
# wrong is handed an artifact it never asked for. The confidential shims are the
# ones with something to lose, since they run a different kernel and image from
# the rest, and qemu-coco-dev is the one that carries such a configuration
# without asking for TEE hardware. (Regression test for #13697)
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

DEFAULT_INSTALL_DIR="/opt/kata"
INSTALLATION_PREFIX="/opt/kata-prefix-test"
INSTALL_DIR="${INSTALLATION_PREFIX}${DEFAULT_INSTALL_DIR}"

CONFIDENTIAL_SHIM="qemu-coco-dev"
PREFIX_POD_NAME="kata-prefix-test"

DROP_IN="10-installation-prefix.toml"
DUMP_SEPARATOR="===DROP-IN==="
DUMP_DIR="${BATS_FILE_TMPDIR:-/tmp}/configs"

# The keys naming an artifact the runtime resolves before it can boot a VM.
ARTIFACT_KEYS=( \
	"kernel" \
	"image" \
	"initrd" \
	"firmware" \
	"firmware_volume" \
	"virtio_fs_daemon" \
)

setup_file() {
	ensure_helm

	echo "# Image: ${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" >&3
	echo "# Hypervisor: ${KATA_HYPERVISOR}" >&3
	echo "# K8s distribution: ${KUBERNETES}" >&3
	echo "# Deploying kata-deploy into ${INSTALL_DIR}..." >&3

	local values
	values=$(mktemp)
	cat > "${values}" <<EOF
env:
  installationPrefix: "${INSTALLATION_PREFIX}"

shims:
  ${CONFIDENTIAL_SHIM}:
    enabled: true
EOF

	deploy_kata "${values}"
	rm -f "${values}"
	echo "# kata-deploy deployed successfully" >&3

	# Reading the host costs a pod per file, so collect the configurations once
	# and let the tests work off the copies.
	mkdir -p "${DUMP_DIR}"

	local dir
	for dir in $(runtime_config_dirs); do
		echo "# Collecting the configuration of ${dir##*/}..." >&3
		runtime_configs "${dir}" > "${DUMP_DIR}/${dir##*/}"
	done
}

# The runtime configuration directories the install left on the host, one per
# line: one per shim, plus one per custom runtime handler derived from a shim.
runtime_config_dirs() {
	run_on_host "find /host${INSTALL_DIR}/share/defaults/kata-containers -maxdepth 4 -type d -name config.d" \
		| sed -e 's|/config.d$||' -e 's|^/host||'
}

# A runtime directory's base configuration followed by its installation prefix
# drop-in, the two separated by ${DUMP_SEPARATOR}. Chained on success so that a
# file that is not there fails the collection rather than handing the tests an
# empty configuration they would find nothing to complain about in.
runtime_configs() {
	local dir="${1}"

	run_on_host "cat /host${dir}/configuration-*.toml && echo ${DUMP_SEPARATOR} && cat /host${dir}/config.d/${DROP_IN}"
}

# The value a configuration gives to a key, empty when it gives none.
toml_value() {
	local content="${1}" key="${2}"

	echo "${content}" | sed -n "s/^${key} = \"\(.*\)\"\$/\1/p" | head -1
}

# Every absolute path a configuration sets, one per line.
toml_paths() {
	echo "${1}" | sed -n 's|^[a-z_]\+ = "\(/[^"]*\)"$|\1|p'
}

@test "Artifacts are installed under the prefix" {
	run run_on_host "test -f /host${INSTALL_DIR}/bin/containerd-shim-kata-v2 && echo FOUND || (test -f /host${INSTALL_DIR}/runtime-rs/bin/containerd-shim-kata-v2 && echo FOUND || echo MISSING)"
	echo "# ${INSTALL_DIR}/bin/containerd-shim-kata-v2: ${output}" >&3
	[[ "${output}" == *"FOUND"* ]]
}

@test "The prefix drop-in moves the artifacts each configuration asks for" {
	local names
	names=$(ls "${DUMP_DIR}")
	[[ -n "${names}" ]]

	local saw_confidential="no"
	local name
	for name in ${names}; do
		if [[ "${name}" == "${CONFIDENTIAL_SHIM}" ]]; then
			saw_confidential="yes"
		fi

		local configs base drop_in
		configs=$(cat "${DUMP_DIR}/${name}")
		base="${configs%%"${DUMP_SEPARATOR}"*}"
		drop_in="${configs##*"${DUMP_SEPARATOR}"}"

		# Every shipped configuration puts its hypervisor under the install
		# directory, so failing to read that is how this notices it is
		# comparing against a configuration it never got.
		[[ "$(toml_value "${base}" "path")" == "${DEFAULT_INSTALL_DIR}/"* ]]

		local key
		for key in "${ARTIFACT_KEYS[@]}"; do
			local base_value drop_in_value
			base_value=$(toml_value "${base}" "${key}")
			drop_in_value=$(toml_value "${drop_in}" "${key}")

			if [[ "${base_value}" == "${DEFAULT_INSTALL_DIR}/"* ]]; then
				echo "# ${name}: ${key} = ${drop_in_value}" >&3
				[[ "${drop_in_value}" == "${INSTALL_DIR}${base_value#"${DEFAULT_INSTALL_DIR}"}" ]]
			else
				# There is nothing of ours to move, so the drop-in has no
				# business naming the key: an initrd handed to an image-only
				# guest is refused outright, and an artifact the install never
				# laid down is a path that cannot resolve.
				[[ -z "${drop_in_value}" ]]
			fi
		done
	done

	# A deploy that quietly left the confidential shim out would satisfy every
	# assertion above without testing what this suite is here for.
	echo "# ${CONFIDENTIAL_SHIM} configured: ${saw_confidential}" >&3
	[[ "${saw_confidential}" == "yes" ]]
}

@test "Every path the prefix drop-in sets is on the host" {
	# The runtime resolves each of them before it can start a VM, so a path
	# that is not there is a boot failure whatever the drop-in says.
	local checks=""
	local name
	for name in $(ls "${DUMP_DIR}"); do
		local configs drop_in path
		configs=$(cat "${DUMP_DIR}/${name}")
		drop_in="${configs##*"${DUMP_SEPARATOR}"}"

		for path in $(toml_paths "${drop_in}"); do
			echo "# ${name}: ${path}" >&3
			checks+="test -e /host${path} || echo MISSING ${path}; "
		done
	done

	[[ -n "${checks}" ]]

	run run_on_host "${checks}true"
	echo "# ${output}" >&3
	[[ "${output}" != *"MISSING"* ]]
}

@test "A pod runs on a relocated installation" {
	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${PREFIX_POD_NAME}
spec:
  runtimeClassName: kata-${KATA_HYPERVISOR}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: test
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: IfNotPresent
      command: ["sleep", "infinity"]
EOF

	echo "# Waiting for the pod to be running..." >&3
	if ! kubectl wait --for=condition=Ready "pod/${PREFIX_POD_NAME}" --timeout=180s; then
		echo "::group::${PREFIX_POD_NAME}"
		kubectl describe "pod/${PREFIX_POD_NAME}" || true
		echo "::endgroup::"
		return 1
	fi
}

teardown_file() {
	kubectl delete pod "${PREFIX_POD_NAME}" --ignore-not-found=true --wait=false 2>/dev/null || true
	uninstall_kata 2>/dev/null || true
}
