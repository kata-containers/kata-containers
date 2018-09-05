#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source /etc/os-release || source /usr/lib/os-release
source "${cidir}/lib.sh"

check_gopath
apply_depends_on

arch=$("${cidir}"/kata-arch.sh -d)
INSTALL_KATA="${INSTALL_KATA:-yes}"

echo "Set up environment"
if [ "$ID" == ubuntu ];then
	bash -f "${cidir}/setup_env_ubuntu.sh"
elif [ "$ID" == fedora ];then
	bash -f "${cidir}/setup_env_fedora.sh"
elif [ "$ID" == centos ];then
	bash -f "${cidir}/setup_env_centos.sh"
else
	die "ERROR: Unrecognised distribution."
	exit 1
fi

if ! command -v docker > /dev/null; then
        "${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker install
fi

# If on CI, check that docker version is the one defined
# in versions.yaml. If there is a different version installed,
# install the correct version..
docker_version=$(get_version "externals.docker.version")
docker_version=${docker_version/v}
docker_version=${docker_version/-*}
if ! sudo docker version | grep -q "$docker_version" && [ "$CI" == true ]; then
	"${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker install -f
fi

case "$arch" in
	x86_64)
	if [ "$CI" == true ] && grep -q "N" /sys/module/kvm_intel/parameters/nested 2> /dev/null; then
		echo "enable Nested Virtualization"
		sudo modprobe -r kvm_intel
		sudo modprobe kvm_intel nested=1
		if grep -q "N" /sys/module/kvm_intel/parameters/nested 2> /dev/null; then
			die "Failed to find or enable Nested virtualization"
		fi
	fi
	;;
	aarch64)
	info "CI running in bare machine"
	;;
	*)
	die "Unsupported architecture: $arch"
	;;
esac

if [ "${INSTALL_KATA}" == "yes" ];then
	echo "Install Kata sources"
	bash -f ${cidir}/install_kata.sh
fi

echo "Install CNI plugins"
bash -f "${cidir}/install_cni_plugins.sh"

# values indicating whether related intergration tests have been supported
CRIO="true"
KUBERNETES="true"
OPENSHIFT="true"

# load arch-specific lib file
if [ -f "${cidir}/${arch}/lib_setup_${arch}.sh" ]; then
        source "${cidir}/${arch}/lib_setup_${arch}.sh"
fi

[ "${CRIO}" = "true" ] && echo "Install CRI-O" && bash -f "${cidir}/install_crio.sh"

[ "${KUBERNETES}" = "true" ] && echo "Install Kubernetes" && bash -f "${cidir}/install_kubernetes.sh"

[ "${OPENSHIFT}" = "true" ] && echo "Install Openshift" && bash -f "${cidir}/install_openshift.sh"

echo "Disable systemd-journald rate limit"
sudo crudini --set /etc/systemd/journald.conf Journal RateLimitInterval 0s
sudo crudini --set /etc/systemd/journald.conf Journal RateLimitBurst 0
sudo systemctl restart systemd-journald

echo "Drop caches"
sync
sudo -E PATH=$PATH bash -c "echo 3 > /proc/sys/vm/drop_caches"
