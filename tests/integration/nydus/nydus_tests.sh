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
source "${dir_path}/../../lib/common.bash"
source "${dir_path}/../../.ci/lib.sh"
source "/etc/os-release" || source "/usr/lib/os-release"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

need_restore_kata_config=false
kata_config_backup="/tmp/kata-configuration.toml"
SYSCONFIG_FILE="/etc/kata-containers/configuration.toml"
DEFAULT_CONFIG_FILE="/opt/kata/share/defaults/kata-containers/configuration-qemu.toml"
CLH_CONFIG_FILE="/opt/kata/share/defaults/kata-containers/configuration-clh.toml"
DB_CONFIG_FILE="/opt/kata/share/defaults/kata-containers/configuration-dragonball.toml"
need_restore_containerd_config=false
containerd_config="/etc/containerd/config.toml"
containerd_config_backup="/tmp/containerd.config.toml"

# test image for container
IMAGE="${IMAGE:-ghcr.io/dragonflyoss/image-service/alpine:nydus-latest}"

if [ "$KATA_HYPERVISOR" != "qemu" ] && [ "$KATA_HYPERVISOR" != "cloud-hypervisor" ] && [ "$KATA_HYPERVISOR" != "dragonball" ]; then
	echo "Skip nydus test for $KATA_HYPERVISOR, it only works for QEMU/CLH/DB now."
	exit 0
fi

arch="$(uname -m)"
if [ "$arch" != "x86_64" ]; then
	echo "Skip nydus test for $arch, it only works for x86_64 now. See https://github.com/kata-containers/tests/issues/4445"
	exit 0
fi

function install_from_tarball() {
	local package_name="$1"
	local binary_name="$2"
	[ -n "$package_name" ] || die "need package_name"
	[ -n "$binary_name" ] || die "need package release binary_name"

	local url=$(get_version "externals.${package_name}.url")
	local version=$(get_version "externals.${package_name}.version")
	local tarball_url="${url}/releases/download/${version}/${binary_name}-${version}-$arch.tgz"
	if [ "${package_name}" == "nydus" ]; then
		local goarch="$(${dir_path}/../../.ci/kata-arch.sh --golang)"
		tarball_url="${url}/releases/download/${version}/${binary_name}-${version}-linux-$goarch.tgz"
	fi
	echo "Download tarball from ${tarball_url}"
	curl -Ls "$tarball_url" | sudo tar xfz - -C /usr/local/bin --strip-components=1
}

function setup_nydus() {
	# install nydus
	install_from_tarball "nydus" "nydus-static"

	# install nydus-snapshotter
	install_from_tarball "nydus-snapshotter" "nydus-snapshotter"

	# Config nydus snapshotter
	sudo -E cp "$dir_path/nydusd-config.json" /etc/

	# start nydus-snapshotter
	nohup /usr/local/bin/containerd-nydus-grpc \
		--config-path /etc/nydusd-config.json \
		--shared-daemon \
		--log-level debug \
		--root /var/lib/containerd/io.containerd.snapshotter.v1.nydus \
		--cache-dir /var/lib/nydus/cache \
		--nydusd-path /usr/local/bin/nydusd \
		--nydusimg-path /usr/local/bin/nydus-image \
		--disable-cache-manager true \
		--enable-nydus-overlayfs true \
		--log-to-stdout >/dev/null 2>&1 &
}

function config_kata() {
	sudo mkdir -p /etc/kata-containers
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
      [plugins.cri.containerd.runtimes.kata]
         runtime_type = "io.containerd.kata.v2"
         privileged_without_host_devices = true
EOF
}

function setup() {
	setup_nydus
	config_kata
	config_containerd
	restart_containerd_service
	check_processes
	extract_kata_env
}

function run_test() {
	sudo -E crictl pull "${IMAGE}"
	pod=$(sudo -E crictl runp -r kata $dir_path/nydus-sandbox.yaml)
	echo "Pod $pod created"
	cnt=$(sudo -E crictl create $pod $dir_path/nydus-container.yaml $dir_path/nydus-sandbox.yaml)
	echo "Container $cnt created"
	sudo -E crictl start $cnt
	echo "Container $cnt started"

	# ensure container is running
	state=$(sudo -E crictl inspect $cnt | jq .status.state | tr -d '"')
	[ $state == "CONTAINER_RUNNING" ] || die "Container is not running($state)"
	# run a command in container
	crictl exec $cnt ls

	# cleanup containers
	sudo -E crictl stop $cnt
	sudo -E crictl stopp $pod
	sudo -E crictl rmp $pod
}

function teardown() {
	echo "Running teardown"

	# kill nydus-snapshotter
	bin=containerd-nydus-grpc
	kill -9 $(pidof $bin) || true
	[ "$(pidof $bin)" == "" ] || die "$bin is running"

	bin=nydusd
	kill -9 $(pidof $bin) || true
	[ "$(pidof $bin)" == "" ] || die "$bin is running"

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

	clean_env_ctr
	check_processes
}

trap teardown EXIT

echo "Running setup"
setup

echo "Running nydus integration tests"
run_test
