#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly project="kata-containers"
readonly prefix="/opt/kata"
readonly project_to_attach="github.com/${project}/runtime"
readonly tmp_dir=$(mktemp -d -t static-build-tmp.XXXXXXXXXX)
readonly GOPATH="${tmp_dir}/go"
# flag to decide if push tarball to github
push=false
export GOPATH
workdir="${WORKDIR:-$PWD}"

exit_handler() {
	[ -d "${tmp_dir}" ] || sudo rm -rf "${tmp_dir}"
}
trap exit_handler EXIT

projects=(
	proxy
	runtime
	shim
)

die() {
	msg="$*"
	echo "ERROR: ${msg}" >&2
	exit 1
}

info() {
	echo "INFO: $*"
}

usage() {
	return_code=${1:-0}
	cat <<EOT
This script is used as part of the ${project} release process.
It is used to create a tarball with static binaries.


Usage:
${script_name} <options> [version]

Args:
version: The kata version that will be use to create the tarball

options:

-h      : Show this help
-p      : push tarball to ${project_to_attach}
-w <dir>: directory where tarball will be created


EOT

	exit "${return_code}"
}

#Install guest image/initrd asset
install_image() {
	image_destdir="${destdir}/${prefix}/share/kata-containers/"
	info "Create image"
	image_tarball=$(find . -name 'kata-containers-'"${kata_version}"'-*.tar.gz')
	[ -f "${image_tarball}" ] || "${script_dir}/../obs-packaging/kata-containers-image/build_image.sh" -v "${kata_version}"
	image_tarball=$(find . -name 'kata-containers-'"${kata_version}"'-*.tar.gz')
	[ -f "${image_tarball}" ] || die "image not found"
	info "Install image in destdir ${image_tarball}"
	mkdir -p "${image_destdir}"
	tar xf "${image_tarball}" -C "${image_destdir}"
	pushd "${destdir}/${prefix}/share/kata-containers/" >>/dev/null
	info "Create image default symlinks"
	image=$(find . -name 'kata-containers-image*.img')
	initrd=$(find . -name 'kata-containers-initrd*.initrd')
	ln -sf "${image}" kata-containers.img
	ln -sf "${initrd}" kata-containers-initrd.img
	popd >>/dev/null
}

#Install kernel asset
install_kernel() {
	go get "github.com/${project}/packaging" || true
	(
		cd ${GOPATH}/src/github.com/${project}/packaging >>/dev/null
		git checkout "${kata_version}-kernel-config" ||
		git checkout "${kata_version}"

		info "build kernel"
		./kernel/build-kernel.sh setup
		./kernel/build-kernel.sh build
		info "install kernel"
		DESTDIR="${destdir}" PREFIX="${prefix}" ./kernel/build-kernel.sh install
	)
}

#Install experimental kernel asset
install_experimental_kernel() {
	pushd ${GOPATH}/src/github.com/${project}/packaging
	info "build experimental kernel"
	./kernel/build-kernel.sh -e setup
	./kernel/build-kernel.sh -e build
	info "install experimental kernel"
	DESTDIR="${destdir}" PREFIX="${prefix}" ./kernel/build-kernel.sh -e install
	popd
}

# Install static nemu asset
install_nemu() {
	info "build static nemu"
	"${script_dir}/../static-build/nemu/build-static-nemu.sh"
	info "Install static nemu"
	tar xf kata-nemu-static.tar.gz -C "${destdir}"
}

# Install static qemu asset
install_qemu() {
	info "build static qemu"
	"${script_dir}/../static-build/qemu/build-static-qemu.sh"
	info "Install static qemu"
	tar xf kata-qemu-static.tar.gz -C "${destdir}"
}

# Install static qemu-virtiofsd asset
install_qemu_virtiofsd() {
	info "build static qemu-virtiofs"
	"${script_dir}/../static-build/qemu-virtiofs/build-static-qemu-virtiofs.sh"
	info "Install static qemu-virtiofs"
	tar xf kata-qemu-static.tar.gz -C "${destdir}"
}

# Install static firecracker asset
install_firecracker() {
	info "build static firecracker"
	[ -f "firecracker/firecracker-static" ] || "${script_dir}/../static-build/firecracker/build-static-firecracker.sh"
	info "Install static firecracker"
	mkdir -p "${destdir}/opt/kata/bin/"
	sudo install -D --owner root --group root --mode 0744  firecracker/firecracker-static "${destdir}/opt/kata/bin/firecracker"
	sudo install -D --owner root --group root --mode 0744  firecracker/jailer-static "${destdir}/opt/kata/bin/jailer"

}

install_docker_config_script() {
	local docker_config_script_name="kata-configure-docker.sh"
	local docker_config_script="${script_dir}/../static-build/scripts/${docker_config_script_name}"

	local script_dest_dir="${destdir}/opt/kata/share/scripts"

	mkdir -p "$script_dest_dir"

	sudo install --owner root --group root --mode 0755 \
		"$docker_config_script" "$script_dest_dir"
}

#Install all components that are not assets
install_kata_components() {
	for p in "${projects[@]}"; do
		echo "Download ${p}"
		go get "github.com/${project}/$p" || true
		pushd "${GOPATH}/src/github.com/${project}/$p" >>/dev/null
		echo "Checkout to version ${kata_version}"
		git checkout "${kata_version}"
		echo "Build"
		make \
			PREFIX="${prefix}" \
			QEMUCMD="qemu-system-x86_64"
		echo "Install"
		make PREFIX="${prefix}" \
			DESTDIR="${destdir}" \
			install
		popd >>/dev/null
	done
	sed -i -e '/^initrd =/d' "${destdir}/${prefix}/share/defaults/${project}/configuration-qemu.toml"
	sed -i -e '/^initrd =/d' "${destdir}/${prefix}/share/defaults/${project}/configuration-fc.toml"
	pushd "${destdir}/${prefix}/share/defaults/${project}"
	ln -sf "configuration-qemu.toml" configuration.toml
	popd

	pushd "${destdir}/${prefix}/bin"
	cat <<EOT | sudo tee kata-fc
#!/bin/bash
${prefix}/bin/kata-runtime --kata-config "${prefix}/share/defaults/${project}/configuration-fc.toml" \$@
EOT
	sudo chmod +x kata-fc

	cat <<EOT | sudo tee kata-qemu
#!/bin/bash
${prefix}/bin/kata-runtime --kata-config "${prefix}/share/defaults/${project}/configuration-qemu.toml" \$@
EOT
	sudo chmod +x kata-qemu

	cat <<EOT | sudo tee kata-nemu
#!/bin/bash
${prefix}/bin/kata-runtime --kata-config "${prefix}/share/defaults/${project}/configuration-nemu.toml" \$@
EOT
	sudo chmod +x kata-nemu

	popd
}

main() {
	while getopts "hpw:" opt; do
		case $opt in
		h) usage 0 ;;
		p) push="true" ;;
		w) workdir="${OPTARG}" ;;
		esac
	done
	shift $((OPTIND - 1))

	kata_version=${1:-}
	[ -n "${kata_version}" ] || usage 1
	info "Requested version: ${kata_version}"

	destdir="${workdir}/kata-static-${kata_version}-$(uname -m)"
	info "DESTDIR ${destdir}"
	mkdir -p "${destdir}"
	install_image
	install_kata_components
	install_kernel
	install_experimental_kernel
	install_qemu
	install_qemu_virtiofsd
	install_nemu
	install_firecracker
	install_docker_config_script

	tarball_name="${destdir}.tar.xz"
	pushd "${destdir}" >>/dev/null
	tar cfJ "${tarball_name}" "./opt"
	popd >>/dev/null
	if [ "${push}" == "true" ]; then
		hub -C "${GOPATH}/src/github.com/${project}/runtime" release edit -a "${tarball_name}" "${kata_version}"
	else
		echo "Wont push the tarball to github use -p option to do it."
	fi
}

main $@
