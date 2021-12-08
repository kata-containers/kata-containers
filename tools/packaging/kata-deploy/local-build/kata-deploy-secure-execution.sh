#!/bin/bash -eu
# Copyright (c) 2021 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

[ -n "${DEBUG:-}" ] && set -x
[ "$(uname -m)" = s390x ] || (echo >&2 "ERROR: Creation of Secure Execution images is currently only supported on s390x." && exit 1)

kata_build_dir=build
cmdline="loglevel=0 panic= scsi_mod.scan=none swiotlb=262144 agent.config_file=/etc/kata-containers/agent.toml"

usage() {
	cat >&2 << EOF
Usage:
  $0 [options]

Options:
  -b <build dir>     : Kata build directory, containing kata-static tarballs
                       Default: "$kata_build_dir"
  -c <kernel cmdline>: Guest kernel command line
                       Default: "$cmdline"
  -h                 : Show this help

Environment variables:
  HKD (required): Secure Execution host key document, generally specific to your machine. See
                  https://www.ibm.com/docs/en/linux-on-systems?topic=tasks-verify-host-key-document
                  for information on how to retrieve and verify this document.
  DEBUG         : If set, display debug information.
EOF
	exit "${1:-0}"
}

while getopts "b:c:h" opt; do
	case $opt in
		b) kata_build_dir="$OPTARG";;
		c) cmdline="$OPTARG";;
		h) usage;;
		*) usage 1;;
	esac
done
shift $(( OPTIND - 1 ))

[ -n "${HKD:-}" ] || (echo >&2 "No host key document specified." && usage 1)
command -v genprotimg || (echo >&2 "genprotimg is not installed. Install s390-tools." && usage 1)

declare hkd_options
eval "for hkd in $HKD; do
	hkd_options+=\"--host-key-document=\\\"\$hkd\\\" \"
done"

tar_path="kata-static.tar.xz"

tarball_content_dir="$PWD/kata-tarball-content"
protimg_content_dir="$PWD/kata-protimg-content"
rm -rf "$tarball_content_dir" "$protimg_content_dir"
mkdir "$tarball_content_dir" "$protimg_content_dir"

pushd "$kata_build_dir"
for c in qemu shim-v2; do
	tar xvf kata-static-$c.tar.xz -C "$tarball_content_dir"
done

for c in kernel rootfs-initrd; do
	tar xvf kata-static-$c.tar.xz -C "$protimg_content_dir"
done
popd

parmfile="$(mktemp --suffix=-cmdline)"
trap 'rm -f "$parmfile"' EXIT
chmod 600 "$parmfile"
echo "$cmdline" > "$parmfile"

protimg_kata_dir="$protimg_content_dir/opt/kata/share/kata-containers"
tarball_kata_dir="$tarball_content_dir/opt/kata/share/kata-containers"
mkdir "$tarball_kata_dir"

eval genprotimg \
	"$hkd_options" \
	--output="$tarball_kata_dir/kata-containers-secure.img" \
	--image="$protimg_kata_dir/vmlinuz.container" \
	--ramdisk="$protimg_kata_dir/kata-containers-initrd.img" \
	--parmfile="$parmfile" \
	--no-verify # TODO add support once available

pushd "$tarball_content_dir"
tar cJvf "$tar_path" .
popd
