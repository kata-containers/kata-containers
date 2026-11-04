#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Run the csi-sanity suite of github.com/kubernetes-csi/csi-test against the
# driver, with a capacity pool configured. The node tests stage and publish
# volumes, which bind mounts and formats them, so this must run as root.
#
# Environment:
#   CSI_SANITY       csi-sanity binary (default: csi-sanity from PATH)
#   CSI_SANITY_ARGS  additional csi-sanity arguments, e.g. -ginkgo.v
#   ARTIFACTS        directory which receives the driver log (optional)

set -o errexit
set -o nounset
set -o pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
driver_dir=$(cd "${script_dir}/../.." && pwd)

: "${CSI_SANITY:=csi-sanity}"
: "${CSI_SANITY_ARGS:=}"
: "${ARTIFACTS:=}"

# Keep the test volumes small: the filesystem which holds the work directory
# must provide them.
test_volume_size=$((64 * 1024 * 1024))
pool="sanity"

work_dir=$(mktemp -d -t csi-kata-directvolume-sanity.XXXXXX)
driver_pid=""

cleanup() {
	local rc=$?
	# Clean up as much as possible and keep the exit code of the test.
	set +o errexit
	if [[ -n "${driver_pid}" ]]; then
		kill "${driver_pid}" 2>/dev/null || true
		wait "${driver_pid}" 2>/dev/null || true
	fi
	if [[ -n "${ARTIFACTS}" ]]; then
		mkdir -p "${ARTIFACTS}"
		cp "${work_dir}/driver.log" "${ARTIFACTS}/csi-sanity-driver.log" 2>/dev/null || true
	elif [[ "${rc}" -ne 0 ]]; then
		echo "driver log:" >&2
		tail -n 200 "${work_dir}/driver.log" >&2 || true
	fi
	# Unmount whatever a failed test left behind before removing it.
	grep -F " ${work_dir}/" /proc/self/mounts | awk '{print $2}' | sort -r | while read -r mnt; do
		umount "${mnt}" || true
	done
	rm -rf "${work_dir}"
	exit "${rc}"
}
trap cleanup EXIT

if [[ "$(id -u)" -ne 0 ]]; then
	echo "csi-sanity must run as root, the node tests mount volumes" >&2
	exit 1
fi

command -v "${CSI_SANITY}" >/dev/null || {
	echo "${CSI_SANITY} not found, install it with" >&2
	echo "  go install github.com/kubernetes-csi/csi-test/v5/cmd/csi-sanity@<version>" >&2
	exit 1
}

echo "Building the driver"
(cd "${driver_dir}" && CGO_ENABLED=0 go build -ldflags "-X main.version=sanity" -o "${work_dir}/directvolplugin" ./cmd/directvolplugin)

endpoint="${work_dir}/csi.sock"
"${work_dir}/directvolplugin" \
	--endpoint="unix://${endpoint}" \
	--nodeid=sanity-node \
	--statedir="${work_dir}/state" \
	--storagepath="${work_dir}/storage" \
	--spdk-rawpath="${work_dir}/spdk-raw" \
	--spdk-vhostpath="${work_dir}/spdk-vhost" \
	--capacity="${pool}=1Ti" \
	--v=5 >"${work_dir}/driver.log" 2>&1 &
driver_pid=$!

for _ in $(seq 50); do
	[[ -S "${endpoint}" ]] && break
	kill -0 "${driver_pid}" 2>/dev/null || { echo "the driver exited" >&2; exit 1; }
	sleep 0.2
done
[[ -S "${endpoint}" ]] || { echo "the driver did not create ${endpoint}" >&2; exit 1; }

cat >"${work_dir}/parameters.yaml" <<EOF
katacontainers.direct.volume/volumetype: directvol
katacontainers.direct.volume/fstype: ext4
katacontainers.direct.volume/capacity-pool: ${pool}
EOF

read -r -a extra_args <<<"${CSI_SANITY_ARGS}"
"${CSI_SANITY}" \
	-csi.endpoint="${endpoint}" \
	-csi.mountdir="${work_dir}/target" \
	-csi.stagingdir="${work_dir}/staging" \
	-csi.testvolumeparameters="${work_dir}/parameters.yaml" \
	-csi.testvolumesize="${test_volume_size}" \
	"${extra_args[@]}"
