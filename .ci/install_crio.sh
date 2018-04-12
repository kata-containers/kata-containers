#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source /etc/os-release

echo "Get CRI-O sources"
crio_repo="github.com/kubernetes-incubator/cri-o"
go get -d "$crio_repo" || true
pushd "${GOPATH}/src/${crio_repo}"

if [ "$ghprbGhRepository" != "${crio_repo/github.com\/}" ]
then
	crio_version=$(get_version "externals.crio.version")
	git fetch
	git checkout "${crio_version}"
fi

# Add link of go-md2man to $GOPATH/bin
GOBIN="$GOPATH/bin"
if [ ! -d "$GOBIN" ]
then
        mkdir -p "$GOBIN"
fi
ln -sf $(command -v go-md2man) "$GOBIN"

echo "Get CRI Tools"
critools_repo="github.com/kubernetes-incubator/cri-tools"
go get "$critools_repo" || true
pushd "${GOPATH}/src/${critools_repo}"
critools_version=$(grep "ENV CRICTL_COMMIT" "${GOPATH}/src/${crio_repo}/Dockerfile" | cut -d " " -f3)
git checkout "${critools_version}"
go install ./cmd/crictl
sudo install "${GOPATH}/bin/crictl" /usr/bin
popd

echo "Installing CRI-O"
make clean
if [ "$ID" == "centos" ] || [ "$ID" == "fedora" ]; then
	# This is necessary to avoid crashing `make` with `No package devmapper found`
	# by disabling the devmapper driver when the library it requires is not installed
	make BUILDTAGS='exclude_graphdriver_devicemapper libdm_no_deferred_remove'
else
	make
fi
make test-binaries
sudo -E PATH=$PATH sh -c "make install"
sudo -E PATH=$PATH sh -c "make install.config"

containers_config_path="/etc/containers"
echo "Copy containers policy from CRI-O repo to $containers_config_path"
sudo mkdir -p "$containers_config_path"
sudo install -m0444 test/policy.json "$containers_config_path"
popd

echo "Install runc for CRI-O"
go get -d github.com/opencontainers/runc
pushd "${GOPATH}/src/github.com/opencontainers/runc"
git checkout "$runc_version"
make
sudo -E install -D -m0755 runc "/usr/local/bin/crio-runc"
popd

crio_config_file="/etc/crio/crio.conf"
echo "Set runc as default runtime in CRI-O for trusted workloads"
sudo sed -i 's/^runtime =.*/runtime = "\/usr\/local\/bin\/crio-runc"/' "$crio_config_file"

echo "Add docker.io registry to pull images"
sudo sed -i 's/^registries = \[/registries = \[ "docker.io"/' "$crio_config_file"

echo "Set manage_network_ns_lifecycle to true"
network_ns_flag="manage_network_ns_lifecycle"

# Check if flag is already defined in the CRI-O config file.
# If it is already defined, then just change the value to true,
# else, add the flag with the value.
if grep "$network_ns_flag" "$crio_config_file"; then
	sudo sed -i "s/^$network_ns_flag.*/$network_ns_flag = true/" "$crio_config_file"
else
	sudo sed -i "/\[crio.runtime\]/a$network_ns_flag = true" "$crio_config_file"
fi

echo "Set Kata containers as default runtime in CRI-O for untrusted workloads"
sudo sed -i 's/default_workload_trust = "trusted"/default_workload_trust = "untrusted"/' "$crio_config_file"
sudo sed -i 's/runtime_untrusted_workload = ""/runtime_untrusted_workload = "\/usr\/local\/bin\/kata-runtime"/' "$crio_config_file"

service_path="/etc/systemd/system"
crio_service_file="${cidir}/data/crio.service"

echo "Install crio service (${crio_service_file})"
sudo install -m0444 "${crio_service_file}" "${service_path}"

echo "Reload systemd services"
sudo systemctl daemon-reload
