#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -u

# NOTE: Some env variables are set in the Dockerfile - those that are
# intended to be over-rideable.
export ROOTFS_DIR=~/src/rootfs
export GOPATH=~/src/go
export PATH=${PATH}:/usr/local/go/bin:${GOPATH}/bin

kata_repo=github.com/kata-containers/kata-containers
kata_repo_path=/kata-containers

log_header() {
	/bin/echo -e "\n\e[1;42m${1}\e[0m"
}

grab_kata_repos()
{
	# Check out all the repos we will use now, so we can try and ensure they use the specified branch
	# Only check out the branch needed, and make it shallow and thus space/bandwidth efficient
	# Use a green prompt with white text for easy viewing
	log_header "Clone and checkout Kata repo"
	[ -d "${kata_repo_path}" ] || git clone --single-branch --branch $KATA_REPO_VERSION --depth=1 https://${kata_repo} ${kata_repo_path}
}

build_kernel()
{
	cd ${kata_repo_path}/tools/packaging/kata-deploy/local-build/
	make kernel-nvidia-gpu-tarball
}

build_rootfs()
{
	# Due to an issue with debootstrap unmounting /proc when running in a
	# --privileged container, change into /proc to keep it from being umounted.
	# This should only be done for Ubuntu and Debian based OS's. Other OS
	# distributions had issues if building the rootfs from /proc

	if [ "${ROOTFS_OS}" == "ubuntu" ]; then
		cd /proc
	fi
	log_header "Build ${ROOTFS_OS} rootfs"
	sudo -E SECCOMP=no EXTRA_PKGS='kmod' ${kata_repo_path}/tools/osbuilder/rootfs-builder/rootfs.sh $ROOTFS_OS
}


copy_outputs()
{
	log_header "Copy kernel and rootfs to the output directory and provide sample configuration files"
	mkdir -p ${OUTPUT_DIR} || true
	sudo cp -r ${kata_repo_path}/tools/packaging/kata-deploy/local-build/build/ $OUTPUT_DIR
	/bin/echo -e "Check the ./output directory for the kernel and rootfs\n"
}

help() {
cat << EOF
Usage: $0 [-h] [options]
   Description:
        This script builds kernel and rootfs artifacts for Kata Containers,
        configured and built to support QAT hardware.
   Options:
        -d,         Enable debug mode
        -h,         Show this help
EOF
}

main()
{
	local check_in_container=${OUTPUT_DIR:-}
	if [ -z "${check_in_container}" ]; then
		echo "Error: 'OUTPUT_DIR' not set" >&2
		echo "$0 should be run using the Dockerfile supplied." >&2
		exit 1
	fi

	local OPTIND
	while getopts "dh" opt;do
		case ${opt} in
		d)
		    set -x
		    ;;
		h)
		    help
		    exit 0;
		    ;;
		?)
		    # parse failure
		    help
		    echo "ERROR: Failed to parse arguments"
		    exit 1
		    ;;
		esac
	done
	shift $((OPTIND-1))

	grab_kata_repos
	#configure_kernel
	build_kernel
	build_rootfs
	#build_qat_drivers
	#add_qat_to_rootfs
	copy_outputs
}

main "$@"
