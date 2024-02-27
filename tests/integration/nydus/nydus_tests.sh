#!/bin/bash
#
# Copyright (c) 2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#
# This will test the nydus feature is working properly

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

dir_path=$(dirname "$0")
source "${dir_path}/../../common.bash"
source "/etc/os-release" || source "/usr/lib/os-release"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

need_restore_kata_config=false
kata_config_backup="/tmp/kata-configuration.toml"
SYSCONFIG_FILE="/etc/kata-containers/configuration.toml"
DEFAULT_CONFIG_FILE="/opt/kata/share/defaults/kata-containers/configuration-qemu.toml"
CLH_CONFIG_FILE="/opt/kata/share/defaults/kata-containers/configuration-clh.toml"
DB_CONFIG_FILE="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration-dragonball.toml"
need_restore_containerd_config=false
containerd_config="/etc/containerd/config.toml"
containerd_config_backup="/tmp/containerd.config.toml"

# test image for container
IMAGE="${IMAGE:-ghcr.io/dragonflyoss/image-service/alpine:nydus-latest}"

if [ "$KATA_HYPERVISOR" != "qemu" ] && [ "$KATA_HYPERVISOR" != "clh" ] && [ "$KATA_HYPERVISOR" != "dragonball" ]; then
	echo "Skip nydus test for $KATA_HYPERVISOR, it only works for QEMU/CLH/DB now."
	exit 0
fi

case "$KATA_HYPERVISOR" in
	dragonball)
		SYSCONFIG_FILE="/etc/kata-containers/runtime-rs/configuration.toml"
		;;
	*)
		;;
esac

function setup_nydus() {
	# Config nydus snapshotter
	sudo -E cp "$dir_path/nydusd-config.json" /etc/
	sudo -E cp "$dir_path/snapshotter-config.toml" /etc/

	# start nydus-snapshotter
	sudo nohup /usr/local/bin/containerd-nydus-grpc \
		--config /etc/snapshotter-config.toml \
		--nydusd-config /etc/nydusd-config.json &
}

function config_kata() {
	sudo mkdir -p $(dirname $SYSCONFIG_FILE)
	if [ -f "$SYSCONFIG_FILE" ]; then
		need_restore_kata_config=true
		sudo cp -a "${SYSCONFIG_FILE}" "${kata_config_backup}"
	elif [ "$KATA_HYPERVISOR" == "qemu" ]; then
		sudo cp -a "${DEFAULT_CONFIG_FILE}" "${SYSCONFIG_FILE}"
	elif [ "$KATA_HYPERVISOR" == "dragonball" ]; then
		sudo cp -a "${DB_CONFIG_FILE}" "${SYSCONFIG_FILE}"
	else
		sudo cp -a "${CLH_CONFIG_FILE}" "${SYSCONFIG_FILE}"
	fi

	echo "Enabling all debug options in file ${SYSCONFIG_FILE}"
	sudo sed -i -e 's/^#\(enable_debug\).*=.*$/\1 = true/g' "${SYSCONFIG_FILE}"
	sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug"/g' "${SYSCONFIG_FILE}"

	if [ "$KATA_HYPERVISOR" != "dragonball" ]; then
		sudo sed -i 's|^shared_fs.*|shared_fs = "virtio-fs-nydus"|g' "${SYSCONFIG_FILE}"
		sudo sed -i 's|^virtio_fs_daemon.*|virtio_fs_daemon = "/usr/local/bin/nydusd"|g' "${SYSCONFIG_FILE}"
	fi

	sudo sed -i 's|^virtio_fs_extra_args.*|virtio_fs_extra_args = []|g' "${SYSCONFIG_FILE}"
}

function config_containerd() {
	readonly runc_path=$(command -v runc)
	sudo mkdir -p /etc/containerd/
	if [ -f "$containerd_config" ]; then
		need_restore_containerd_config=true
		sudo cp -a "${containerd_config}" "${containerd_config_backup}"
	else
		sudo rm "${containerd_config}"
	fi

	cat <<EOF | sudo tee $containerd_config
[debug]
  level = "debug"
[proxy_plugins]
  [proxy_plugins.nydus]
    type = "snapshot"
    address = "/run/containerd-nydus/containerd-nydus-grpc.sock"
[plugins]
  [plugins.cri]
    disable_hugetlb_controller = false
    [plugins.cri.containerd]
      snapshotter = "nydus"
      disable_snapshot_annotations = false
      [plugins.cri.containerd.runtimes]
      [plugins.cri.containerd.runtimes.runc]
         runtime_type = "io.containerd.runc.v2"
         [plugins.cri.containerd.runtimes.runc.options]
           BinaryName = "${runc_path}"
           Root = ""
      [plugins.cri.containerd.runtimes.kata-${KATA_HYPERVISOR}]
         runtime_type = "io.containerd.kata-${KATA_HYPERVISOR}.v2"
         privileged_without_host_devices = true
EOF
}

function check_nydus_snapshotter_exist() {
	echo "check_nydus_snapshotter_exist"
	bin="containerd-nydus-grpc"
	if pgrep -f "$bin" >/dev/null; then
		echo "nydus-snapshotter is running"
	else
		die "nydus-snapshotter is not running"
	fi
}

function setup() {
	setup_nydus
	config_kata
	config_containerd
	restart_containerd_service
	check_processes
	check_nydus_snapshotter_exist
	extract_kata_env
}

function run_test() {
	sudo -E crictl --timeout=20s pull "${IMAGE}"
	pod=$(sudo -E crictl --timeout=20s runp -r kata-${KATA_HYPERVISOR} $dir_path/nydus-sandbox.yaml)
	echo "Pod $pod created"
	cnt=$(sudo -E crictl --timeout=20s create $pod $dir_path/nydus-container.yaml $dir_path/nydus-sandbox.yaml)
	echo "Container $cnt created"
	sudo -E crictl --timeout=20s start $cnt
	echo "Container $cnt started"

	# ensure container is running
	state=$(sudo -E crictl --timeout=20s inspect $cnt | jq .status.state | tr -d '"')
	[ $state == "CONTAINER_RUNNING" ] || die "Container is not running($state)"
	# run a command in container
	sudo -E crictl --timeout=20s exec $cnt ls

	# cleanup containers
	sudo -E crictl --timeout=20s stop $cnt
	sudo -E crictl --timeout=20s stopp $pod
	sudo -E crictl --timeout=20s rmp $pod
}

function teardown() {
	echo "Running teardown"
	local rc=0

	local pid
	for bin in containerd-nydus-grpc nydusd; do
		pid=$(pidof $bin)
		if [ -n "$pid" ]; then
			echo "Killing $bin processes"
			# shellcheck disable=SC2086
			sudo -E kill -9 $pid || true
			if [ -n "$(pidof $bin)" ]; then
				echo "$bin is still running ($pid) but it should not"
				rc=1
			fi
		fi
	done

	# restore kata configuratiom.toml if needed
	if [ "${need_restore_kata_config}" == "true" ]; then
		sudo mv "$kata_config_backup" "$SYSCONFIG_FILE"
	else
		sudo rm "$SYSCONFIG_FILE"
	fi

	# restore containerd config.toml if needed
	if [ "${need_restore_containerd_config}" == "true" ]; then
		sudo mv "$containerd_config_backup" "$containerd_config"
	else
		sudo rm "$containerd_config"
	fi

	clean_env_ctr || rc=1
	check_processes
	return $rc
}

trap teardown EXIT

echo "Running setup"
setup

echo "Running nydus integration tests"
run_test
