#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

error(){
	msg="$*"
	echo "ERROR: $msg" >&2
}

die(){
	error "$*"
	exit 1
}

make_random_ip_addr() {
	echo "127.$((1 + RANDOM % 240)).$((1 + RANDOM % 240)).$((1 + RANDOM % 240))"
}

make_random_port() {
	echo "$((11060 + RANDOM % 1000))"
}

get_dnssearch() {
	echo "$(grep search /etc/resolv.conf | cut -d' ' -f 2)"
}

get_dns() {
	v="$(grep nameserver /etc/resolv.conf | cut -d' ' -f2 | sed -e 's/^/"/g' -e 's/$/",/g')"
	echo ${v} | sed -e 's|,$||g'
}

download() {
	url="$1"
	outdir="$2"
	pushd "${outdir}"
	curl -LO ${url}
	ret=$?
	popd
	return ${ret}
}

setup_image() {
	img_url=$1
	img=$2
	[ -f "${img}" ] && return
	{ download "${img_url}" "$(dirname ${img})"; ret=$?; } || true
	[ ${ret} != 0 ] && rm -f "${img}" && return
	qemu-img resize "${img}" +5G
}

# arg1: ip
# arg2: port
# arg3: ssh key
# arg4: timeout in minutes
# return: 0 on success, 1 otherwise
ping_vm() {
	ip="$1"
	port="$2"
	sshkeyfile="$3"
	timeout=$4
	minute=60
	sleeptime=10
	timeoutsec=$((timeout*minute))
	tries=$((timeoutsec/sleeptime))

	for i in $(seq 1 ${tries}); do
		ssh -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no -o IdentitiesOnly=yes -i "${sshkeyfile}" "${ip}" -p "${port}" true && return 0
		sleep ${sleeptime}
	done

	return 1
}

# arg1: qemu system: ppc64, aarch64 or x86_64
# arg2: cpu model
# arg3: machine type
# arg4: ip
# arg5: port
# arg6: image path
# arg7: seed image path
# arg8: extra options
run_qemu() {
	local arch="${1}"
	local cpu="${2}"
	local machine="${3}"
	local ip="${4}"
	local port="${5}"
	local image="${6}"
	local seed_img="${7}"
	local extra_opts="${8}"
	local ssh_key_file="id_rsa"
	local ping_timeout=15

	local img_opts="-drive file=${image},if=virtio,format=qcow2,aio=threads"
	local seed_opts="-drive file=${seed_img},if=virtio,media=cdrom"
	if [ "${arch}" == "aarch64" ]; then
		img_opts="-device virtio-blk-device,drive=image -drive file=${image},if=none,id=image,aio=threads"
		seed_opts="-device virtio-blk-device,drive=cloud -drive file=${seed_img},if=none,id=cloud,format=raw"
	fi

	qemu-system-${arch} -cpu "${cpu}" -machine "${machine}" -smp cpus=4 -m 2048M \
				-net nic,model=virtio -device virtio-rng-pci -net user,hostfwd=tcp:${ip}:${port}-:22,dnssearch="$(get_dnssearch)" \
				${img_opts} ${seed_opts} \
				-display none -vga none -daemonize ${extra_opts}
	[ $? != 0 ] && return 1

	# depending of the host's hw, it takes for around ~15 minutes
	ping_vm "${ip}" "${port}" "${ssh_key_file}" ${ping_timeout}
	[ $? != 0 ] && return 1

	return 0
}
