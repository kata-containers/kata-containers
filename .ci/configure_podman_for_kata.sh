#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")
source  "${cidir}/../lib/common.bash"

default_kata_config="/usr/share/defaults/kata-containers/configuration.toml"
kata_config="/etc/kata-containers/configuration.toml"
TEST_INITRD="${TEST_INITRD:-no}"
TRUSTED_GROUP="${TRUSTED_GROUP:-kvm}"

setup_prerequisites() {
	# Verify host kernel version
	host_kernel_version=$(uname -r|cut -d. -f1-2)
	kernel_version="4.14"
	result=$(echo "${host_kernel_version} >= ${kernel_version}"|bc)
	[ "${result}" -ne 1 ] && die "Host kernel version is ${host_kernel_version} which is too old"
	# Disable selinux
	if  [ "$(getenforce)" != "Disabled" ]; then
		sudo setenforce 0
		sudo sed -i 's/^SELINUX=enforcing/SELINUX=disabled/g' /etc/sysconfig/selinux
	fi
	# Add user to KVM
	getent group "${TRUSTED_GROUP}" &>/dev/null || sudo groupadd --system "${TRUSTED_GROUP}"
	sudo usermod -a -G "${TRUSTED_GROUP}" $USER
	newgrp "${TRUSTED_GROUP}" << END
	echo "This is running as group $(id -gn)"
END
	sudo chown root:"${TRUSTED_GROUP}" /dev/"${TRUSTED_GROUP}"
	sudo chmod g+rw /dev/"${TRUSTED_GROUP}"
}

setup_kata_configuration_files() {
	sudo mkdir -p $(dirname "${kata_config}")
	[ ! -e "${kata_config}" ] && sudo install -D "${default_kata_config}" $(dirname "${kata_config}")

	sudo chown root:"${TRUSTED_GROUP}" "${kata_config}"
	sudo chmod g+r "${kata_config}"
}

disable_vhost_net() {
	sudo sed -i -e 's/^#disable_vhost_net = true/disable_vhost_net = true/' "${kata_config}"
}

modify_kata_image_permissions() {
	if [ "${TEST_INITRD}" == "yes" ]; then
		img=$(readlink -f /usr/share/kata-containers/kata-containers-initrd.img)
	else
		img=$(readlink -f /usr/share/kata-containers/kata-containers.img)
	fi
	sudo chown -R "root:$TRUSTED_GROUP" "$img"
	sudo chmod -R g+rw "$img"
}

kata_runtime_podman() {
	echo 'kata-runtime = ["/usr/local/bin/kata-runtime"]' | sudo tee -a /usr/share/containers/libpod.conf
	sudo sed -i -e 's/^runtime =.*/runtime = "kata-runtime"/' /usr/share/containers/libpod.conf
}

main() {
	setup_prerequisites
	setup_kata_configuration_files
	disable_vhost_net
	modify_kata_image_permissions
	kata_runtime_podman
}
main "$@"
