#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source /etc/os-release || source /usr/lib/os-release

echo "Install go-md2man"
go_md2man_url=$(get_test_version "externals.go-md2man.url")
go_md2man_version=$(get_test_version "externals.go-md2man.version")
go_md2man_repo=${go_md2man_url/https:\/\/}
go get -d "${go_md2man_repo}"
pushd "$GOPATH/src/${go_md2man_repo}"
git checkout "${go_md2man_version}"
go build
go install
popd

echo "Install conmon"
conmon_url=$(get_version "externals.conmon.url")
conmon_version=$(get_version "externals.conmon.version")
conmon_repo=${conmon_url/https:\/\/}
go get -d "${conmon_repo}" || true
pushd "$GOPATH/src/${conmon_repo}"
git checkout "${conmon_version}"
make
sudo -E make install
popd

echo "Get CRI-O sources"
kubernetes_sigs_org="github.com/kubernetes-sigs"
ghprbGhRepository="${ghprbGhRepository:-}"
crio_repo=$(get_version "externals.crio.url")
# remove https:// from the url
crio_repo="${crio_repo#*//}"
crio_config_file="/etc/crio/crio.conf"

# Remove CRI-O repository if already exists on Fedora
if [ "$ID" == "fedora" ]; then
	if [ -d "${GOPATH}/src/${crio_repo}" ]; then
		sudo rm -r "${GOPATH}/src/${crio_repo}"
	fi
fi

crio_version=$(get_version "externals.crio.version")
crictl_repo=$(get_version "externals.critools.url")
crictl_version=$(get_version "externals.critools.version")
crictl_tag_prefix="v"

go get -d "$crio_repo" || true

if [ "$ghprbGhRepository" != "${crio_repo/github.com\/}" ]
then
	# For Fedora, we use CRI-O version that is compatible with the
	# Openshift version that we support (usually the latest stable).
	# For other distros, we use the CRI-O version that is compatible with
	# the kubernetes version that we support (usually latest stable).
	# Sometimes these versions differ.
	if [ "$ID" == "fedora" ]; then
		if [ "$KUBERNETES" == "yes" ]; then
			crio_version=$(get_version "externals.crio.version")
		else
			crio_version=$(get_version "externals.crio.meta.openshift")
		fi
		crictl_version=$(get_version "externals.crio.meta.crictl")
		crictl_tag_prefix=""
	fi

	# Only fetch and checkout if we are not testing changes in the cri-o repo. 
	pushd "${GOPATH}/src/${crio_repo}"
	git fetch
	git checkout "${crio_version}"
	popd
fi

pushd "${GOPATH}/src/${crio_repo}"
echo "Installing CRI-O"
make clean
if [ "$ID" == "centos" ] || [ "$ID" == "fedora" ]; then
	# This is necessary to avoid crashing `make` with `No package devmapper found`
	# by disabling the devmapper driver when the library it requires is not installed
	sed -i 's|$(shell hack/selinux_tag.sh)||' Makefile
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

# Install cri-tools
echo "Installing CRI Tools"
crictl_url="${crictl_repo}/releases/download/v${crictl_version}/crictl-${crictl_tag_prefix}${crictl_version}-linux-$(${cidir}/kata-arch.sh -g).tar.gz"
curl -Ls "$crictl_url" | sudo tar xfz - -C /usr/local/bin

# Change socket format and pause image used for infra containers
# Needed for cri-o 1.10
if crio --version | grep '1.10'; then
	sudo sed -i 's|/var|unix:///var|' /etc/crictl.yaml
	sudo sed -i 's|kubernetes/pause|k8s.gcr.io/pause|' "$crio_config_file"
fi

echo "Install runc for CRI-O"
runc_version=$(get_version "externals.runc.version")
go get -d github.com/opencontainers/runc
pushd "${GOPATH}/src/github.com/opencontainers/runc"
git checkout "$runc_version"
make
sudo -E install -D -m0755 runc "/usr/local/bin/crio-runc"
popd

echo "Set manage_network_ns_lifecycle to true"
network_ns_flag="manage_network_ns_lifecycle"
sudo sed -i "/\[crio.runtime\]/a$network_ns_flag = true" "$crio_config_file"
sudo sed -i 's/manage_network_ns_lifecycle = false/#manage_network_ns_lifecycle = false/' "$crio_config_file"

echo "Add docker.io registry to pull images"
# Matches cri-o 1.10 file format
sudo sed -i 's/^registries = \[/registries = \[ "docker.io"/' "$crio_config_file"
# Matches cri-o 1.12 file format
sudo sed -i 's/^#registries = \[/registries = \[ "docker.io" \] /' "$crio_config_file"

service_path="/etc/systemd/system"
crio_service_file="${cidir}/data/crio.service"

echo "Install crio service (${crio_service_file})"
sudo install -m0444 "${crio_service_file}" "${service_path}"

kubelet_service_dir="${service_path}/kubelet.service.d/"

sudo mkdir -p "${kubelet_service_dir}"

cat <<EOF| sudo tee "${kubelet_service_dir}/0-crio.conf"
[Service]
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///var/run/crio/crio.sock"
EOF

echo "Reload systemd services"
sudo systemctl daemon-reload
