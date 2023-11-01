#!/usr/bin/env bash

# //
# // Copyright (c) 2020 Ant Group
# //
# // SPDX-License-Identifier: Apache-2.0
# //

die() {
    cat <<EOF >&2
====================================================================
====                compile protocols failed                    ====

$1

====================================================================
EOF
    exit 1
}

show_succeed_msg() {
    echo "===================================================================="
    echo "====                                                            ===="
    echo "====                compile protocols succeed                   ===="
    echo "====                                                            ===="
    echo "===================================================================="
}

show_usage() {
    echo "===================================================================="
    echo ""
    echo "       USAGE: generate-protocols <FILE|all>"
    echo ""
    echo "       Where the first argument could be:"
    echo "         all: will compile all protocol buffer files"
    echo ""
    echo "         Or compile individually by using the exact proto file:"

    # iterate over proto files
    for file in "$@"
    do
        echo "         $file"
    done

    echo ""
    echo "===================================================================="
}

generate_go_sources() {
    local proto_file="$1"
    local dir_path="${proto_file%/*}"
    local file_name="${proto_file##*/}"

    [ "$dir_path" == "$proto_file" ] && dir_path="."

    local root_path=$(realpath ../)/libs/protocols/protos
    local output_path=$(realpath ../)/runtime/virtcontainers/pkg/agent/protocols/$dir_path
    local mapping="Mgoogle/protobuf/empty.proto=google.golang.org/protobuf/types/known/emptypb"

    local cmd="protoc -I$GOPATH/src:${root_path} \
    --go_out=paths=source_relative,$mapping:$output_path \
    --go-ttrpc_out=paths=source_relative,$mapping:$output_path \
    ${root_path}/$file_name"

    echo $cmd
    $cmd
    [ $? -eq 0 ] || die "Failed to generate golang file from $1"
}

if [ "$(basename $(pwd))" != "agent" ]; then
	die "Please go to root directory of agent before execute this shell"
fi

# Protocol buffer files required to generate golang/rust bindings.
proto_files_list=(grpc/agent.proto grpc/csi.proto grpc/health.proto grpc/oci.proto types.proto)

if [ "$1" = "" ]; then
    show_usage "${proto_files_list[@]}"
    exit 1
fi;

# pre-requirement check
which protoc
[ $? -eq 0 ] || die "Please install protoc from github.com/protocolbuffers/protobuf"

which protoc-gen-gogottrpc
[ $? -eq 0 ] || die "Please install protoc-gen-gogottrpc from https://github.com/containerd/ttrpc"

[[ -n "$GOPATH" ]] || die "GOPATH is not set. Please set it."

# do generate work
target=$1

# compile all proto files
if [ "$target" = "all" ]; then
    # compile all proto files
    for f in ${proto_files_list[@]}; do
        echo -e "\n   [golang] compiling ${f} ..."
        generate_go_sources $f
        echo -e "   [golang] ${f} compiled\n"
    done
else
    # compile individual proto file
    for f in ${proto_files_list[@]}; do
        if [ "$target" = "$f" ]; then
            echo -e "\n   [golang] compiling ${target} ..."
            generate_go_sources $target
            echo -e "   [golang] ${target} compiled\n"
        fi
    done
fi;

# if have no errors, compilation will succeed
show_succeed_msg
