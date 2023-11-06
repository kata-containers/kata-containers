#!/bin/bash
#
# Copyright (c) 2017-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Runtime to be used for testing
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
RUNTIME=${RUNTIME:-containerd-shim-kata-${KATA_HYPERVISOR}-v2}
FACTORY_TEST=${FACTORY_TEST:-""}
USE_DEVMAPPER="${USE_DEVMAPPER:-false}"

readonly default_containerd_config="/etc/containerd/config.toml"
readonly kata_config="/etc/kata-containers/configuration.toml"
readonly kata_config_backup="$kata_config.backup"
readonly default_kata_config="/opt/kata/share/defaults/kata-containers/configuration.toml"

containerd_runtime_type="io.containerd.kata-${KATA_HYPERVISOR}.v2"
containerd_shim_path="$(command -v containerd-shim)"

function ci_config() {
	sudo mkdir -p $(dirname "${kata_config}")
	[ -f "$kata_config" ] && sudo cp "$kata_config" "$kata_config_backup" || \
		sudo cp "$default_kata_config" "$kata_config"

	source /etc/os-release || source /usr/lib/os-release
	ID=${ID:-""}
	if [ "$ID" == ubuntu ]; then
		# https://github.com/kata-containers/tests/issues/352
		if [ -n "${FACTORY_TEST}" ]; then
			sudo sed -i -e 's/^#enable_template.*$/enable_template = true/g' "${kata_config}"
			echo "init vm template"
			sudo -E PATH=$PATH "$RUNTIME" factory init
		fi
	fi

	echo "enable debug for kata-runtime"
	sudo sed -i 's/^#enable_debug =/enable_debug =/g' ${kata_config}
}

function ci_cleanup() {
	source /etc/os-release || source /usr/lib/os-release

	if [ -n "${FACTORY_TEST}" ]; then
		echo "destroy vm template"
		sudo -E PATH=$PATH "$RUNTIME" factory destroy
	fi

	if [ -e "$default_containerd_config_backup" ]; then
		echo "restore containerd config"
		sudo systemctl stop containerd
		sudo cp "$default_containerd_config_backup" "$default_containerd_config"
	fi

	[ -f "$kata_config_backup" ] && sudo mv "$kata_config_backup" "$kata_config" || \
		sudo rm "$kata_config"
}

function create_containerd_config() {
	local runtime="$1"
	# kata_annotations is set to 1 if caller want containerd setup with
	# kata annotations support.
	local kata_annotations=${2-0}
	[ -n "${runtime}" ] || die "need runtime to create config"

	local runtime_type="${containerd_runtime_type}"
	if [ "${runtime}" == "runc" ]; then
		runtime_type="io.containerd.runc.v2"
	fi
	local containerd_runtime=$(command -v "containerd-shim-${runtime}-v2")

cat << EOF | sudo tee "${CONTAINERD_CONFIG_FILE}"
[debug]
  level = "debug"
[plugins]
  [plugins.cri]
    [plugins.cri.containerd]
        default_runtime_name = "$runtime"
      [plugins.cri.containerd.runtimes.${runtime}]
        runtime_type = "${runtime_type}"
        $( [ $kata_annotations -eq 1 ] && \
        echo 'pod_annotations = ["io.katacontainers.*"]' && \
        echo '        container_annotations = ["io.katacontainers.*"]'
        )
        [plugins.cri.containerd.runtimes.${runtime}.options]
          Runtime = "${containerd_runtime}"
[plugins.linux]
       shim = "${containerd_shim_path}"
EOF

if [ "$USE_DEVMAPPER" == "true" ]; then
	sudo sed -i 's|^\(\[plugins\]\).*|\1\n  \[plugins.devmapper\]\n    pool_name = \"contd-thin-pool\"\n    base_image_size = \"4096MB\"|' ${CONTAINERD_CONFIG_FILE}
	echo "Devicemapper configured"
	cat "${CONTAINERD_CONFIG_FILE}"
fi

}

# k8s may restart docker which will impact on containerd stop
function stop_containerd() {
	local tmp=$(pgrep kubelet || true)
	[ -n "$tmp" ] && sudo kubeadm reset -f

	sudo systemctl stop containerd
}
