#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Build in parallel snap images using VMs.
# This script runs in the host.

source lib.sh

readonly supported_archs=(all amd64 ppc64 arm64)

seed_dir=seed
seed_img=seed.img
id_rsa_file=id_rsa
id_rsa_pub_file=id_rsa.pub
snap_sh=snap.sh
ssh="ssh -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no -o IdentitiesOnly=yes -i ${id_rsa_file}"
scp="scp -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no -o IdentitiesOnly=yes -i ${id_rsa_file}"

gen_seed() {
	rm -f "${seed_img}"
	truncate --size 2M "${seed_img}"
	mkfs.vfat -n cidata "${seed_img}" &> /dev/null

	if [ -n "${http_proxy}" ]; then
		apt_proxy="apt:\n  https_proxy: ${https_proxy}\n  proxy: ${http_proxy}"
		docker_env="\"HTTP_PROXY=${http_proxy}\" \"HTTPS_PROXY=${https_proxy}\" \"NO_PROXY=${no_proxy}\""
		env="no_proxy=${no_proxy}\n\
    NO_PROXY=${no_proxy}\n\
    http_proxy=${http_proxy}\n\
    HTTP_PROXY=${http_proxy}\n\
    https_proxy=${https_proxy}\n\
    HTTPS_PROXY=${https_proxy}"
	fi

	docker_dns="$(get_dns)"

	[ ! -f "${id_rsa_file}" ] && ssh-keygen -t rsa -f ${id_rsa_file} -P '' &> /dev/null
	ssh_key="$(cat ${id_rsa_pub_file})"

	sed \
		-e "s|@USER@|""${USER}""|g" \
		-e "s|@SSH_KEY@|""${ssh_key}""|g" \
		-e "s|@APT_PROXY@|""${apt_proxy}""|g" \
		-e "s|@DOCKER_ENV@|""${docker_env}""|g" \
		-e "s|@DOCKER_DNS@|""${docker_dns}""|g" \
		-e "s|@ENV@|""${env}""|g" \
		${seed_dir}/user-data.in > ${seed_dir}/user-data

	mcopy -oi "${seed_img}" ${seed_dir}/user-data  ${seed_dir}/meta-data ::
}

poweroff_and_die() {
	ip="$1"
	port="$2"
	${ssh} "${ip}" -p "${port}" sudo poweroff
	die "$3"
}

build_arch() {
	set -x -e
	local arch="$1"
	source "config_${arch}.sh"
	local ip="$(make_random_ip_addr)"
	local port="$(make_random_port)"

	setup_image "${arch_image_url}" "${arch_image}"

	# download bios if needed
	if [ -n "${arch_bios}" ] && [ -n "${arch_bios_url}" ]; then
		arch_qemu_extra_opts+=" -bios ${arch_bios}"
		[ -f "${arch_bios}" ] || download "${arch_bios_url}" "."
	fi

	# run QEMU
	run_qemu "${arch_qemu}" \
			 "${arch_qemu_cpu}" \
			 "${arch_qemu_machine}" \
			 "${ip}" \
			 "${port}" \
			 "${arch_image}" \
			 "${seed_img}" \
			 "${arch_qemu_extra_opts}"

	# copy snap script to VM
	${scp} -P "${port}" "${snap_sh}" "${ip}:~/" || poweroff_and_die "${ip}" "${port}" "Could not copy snap script"

	# run snap script in the VM
	${ssh} "${ip}" -p "${port}" "~/snap.sh" || poweroff_and_die "${ip}" "${port}" "Failed to run build script"

	# copy snap image from VM
	${scp} -P "${port}" "${ip}:~/packaging/*.snap" . ||  poweroff_and_die "${ip}" "${port}" "Failed to get snap image"

	# poweroff VM
	${ssh} "${ip}" -p "${port}" sudo poweroff
}

help()
{
	usage=$(cat << EOF
Usage: $0 [-h] [options]
  Description:
    Build snap images.
  Options:
    -a <arch>,  Build snap image for all or a specific architecture (mandatory).
    -h,         Show this help text and exit.

  Supported architectures:
    $(IFS=$'\t'; echo -e "${supported_archs[*]}")
EOF
)
	echo "$usage"
}

main() {
	local arch
	local OPTIND
	while getopts "a:h" opt; do
		case ${opt} in
		a)
		    arch="${OPTARG}"
		    ;;
		h)
		    help
		    exit 0;
		    ;;
		?)
		    # parse failure
		    help
		    die "Failed to parse arguments"
		    ;;
		esac
	done
	shift $((OPTIND-1))

	[ -z "${arch}" ] && help && die "Mandatory architecture not supplied"
	if ! [[ " ${supported_archs[@]} " =~ " ${arch} " ]]; then
		help
		die "Architecture '${arch}' not supported"
	fi

	gen_seed

	if [ "${arch}" != "all" ]; then
		build_arch "${arch}" &> "${arch}.log"
	else
		for a in ${supported_archs[@]}; do
			(build_arch "${a}" &> "${a}.log") &
		done
		wait
	fi
}

main "$@"
