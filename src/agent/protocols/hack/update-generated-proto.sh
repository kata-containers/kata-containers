#!/bin/bash

# Copyright 2020 Ant Financial
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

readonly script_name=${0##*/}

# Protocol buffer files required to generate rust bindings.
#
# These files define the Kata Containers agent API.
proto_files_list=(
	"agent.proto"
	"health.proto"
	"oci.proto"
	"github.com/kata-containers/agent/pkg/types/types.proto"
)

# List of required command and their canonical sites.
deps=(
	"protoc:https://github.com/protocolbuffers/protobuf"
	"protoc-gen-rust:https://github.com/pingcap/grpc-rs"
	"ttrpc_rust_plugin:https://github.com/containerd/ttrpc-rust"
)

project_slug="kata-containers/kata-containers"
project_site="github.com/${project_slug}"

die()
{
	echo >&2 "ERROR: $*"
	exit 1
}

info()
{
	echo "INFO: $*"
}

setup()
{
	[ -z "${GOPATH}" ] && die "GOPATH not set"

	local dep

	for dep in "${deps[@]}"
	do
		local cmd=$(echo "$dep"|cut -d: -f1)
		local url=$(echo "$dep"|cut -d: -f2-)

		command -v "$cmd" &>/dev/null || die "Install $cmd from $url"
	done
}

usage()
{
	local file_list=$(echo ${proto_files_list[@]} | sed 's/ /, /g')

	cat <<-EOT
	Usage: $script_name [options] <proto-file>

	Description: Generate Kata Containers agent rust API bindings
	  from protocol buffer definition (.proto) files.

	Options:

	 -h : Show this help statement.

	Notes:

	- <proto-file> can be either:
	  - A file (one of ${file_list}).
	  - The word 'all', meaning process all files.

EOT
}

generate_rust_sources()
{
	local file="${1:-}"

	[ -z "$file" ] && die "need file"

	local filepath="${GOPATH}/src/${project_site}/src/agent/protocols/protos/${file}"

	[ -e "$filepath" ] || die "file $file ($filepath) does not exist"

	local out_dir="./protocols/src"
	local out_file=$(basename "${file/.proto/.rs}")
	local outfile_path="${out_dir}/${out_file}"

	local ttrpc_cmd=$(command -v ttrpc_rust_plugin)

	local includes=()

	# Yes, this *is* required!
	includes+=("${GOPATH}/src")

	includes+=("${GOPATH}/src/${project_site}/src/agent/protocols/protos")

	local include

	for include in "${includes[@]}"
	do
		[ -d "$include" ] || die "include directory '$include' does not exist"
	done

	local include_path=$(echo "${includes[@]}"|tr ' ' ':')

	local cmd="protoc \
		--rust_out=${out_dir} \
		--ttrpc_out=${out_dir},plugins=ttrpc:${out_dir} \
		--plugin=protoc-gen-ttrpc=${ttrpc_cmd} \
		-I${include_path} \
		${filepath}"

	local ret

	info "Converting '$file' into '$out_file'"

	{ $cmd; ret=$?; } || true

	[ $ret -eq 0 ] || die "Failed to generate rust file from ${file}"

	if [ "$file" = "oci.proto" ]
	then
		# Need change Box<Self> to ::std::boxed::Box<Self> because there is another struct Box
		sed -i 's/fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {/fn into_any(self: ::std::boxed::Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {/g' "$outfile_path"
	fi
}

handle_targets()
{
	local targets=("${@}")

	[ -z "$targets" ] && die "need targets"

	local target

	for target in ${targets[@]}
	do
		generate_rust_sources "$target"
	done
}

main()
{
	local target="${1:-}"

	case "$target" in
		-h|--help|help) usage; exit 0 ;;
		"") usage; exit 1 ;;
	esac

	local expected_dir="agent"

	[ "$(basename $(pwd))" != "$expected_dir" ] && \
		die "Run from $expected_dir directory"

	setup

	local targets=()

	if [ "$target" = "all" ]
	then
		targets=("${proto_files_list[@]}")
	else
		targets=("$target")
	fi

	handle_targets "${targets[@]}"

	info "Done"
}

main "$*"
