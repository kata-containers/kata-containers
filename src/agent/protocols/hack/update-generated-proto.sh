#!/bin/bash

# //
# // Copyright 2020 Ant Financial
# //
# // SPDX-License-Identifier: Apache-2.0
# //

die() {
    cat <<EOT >&2
====================================================================
====                compile protocols failed                    ====

$1

====================================================================
EOT
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
    echo "       USAGE: make PROTO_FILE=<xyz.proto> generate-protocols"
    echo ""
    echo "       Where PROTO_FILE may be:"
    echo "         all: will compile all protocol buffer files"
    echo ""
    echo "       Or compile individually by using the exact proto file:"

    # iterate over proto files
    for file in "$@"
    do
        echo "         $file"
    done

    echo ""
    echo "===================================================================="
}

generate_go_sources() {
    local cmd="protoc -I$GOPATH/src/github.com/kata-containers/agent/vendor/github.com/gogo/protobuf:$GOPATH/src/github.com/kata-containers/agent/vendor:$GOPATH/src/github.com/gogo/protobuf:$GOPATH/src/github.com/gogo/googleapis:$GOPATH/src:$GOPATH/src/github.com/kata-containers/kata-containers/src/agent/protocols/protos \
--gogottrpc_out=plugins=ttrpc+fieldpath,\
import_path=github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc,\
\
Mgithub.com/kata-containers/kata-containers/src/agent/protocols/protos/github.com/kata-containers/agent/pkg/types/types.proto=github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols,\
\
Mgithub.com/kata-containers/kata-containers/src/agent/protocols/protos/oci.proto=github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc,\
\
Mgogoproto/gogo.proto=github.com/gogo/protobuf/gogoproto,Mgoogle/protobuf/any.proto=github.com/gogo/protobuf/types,Mgoogle/protobuf/descriptor.proto=github.com/gogo/protobuf/protoc-gen-gogo/descriptor,Mgoogle/protobuf/duration.proto=github.com/gogo/protobuf/types,Mgoogle/protobuf/empty.proto=github.com/gogo/protobuf/types,Mgoogle/protobuf/field_mask.proto=github.com/gogo/protobuf/types,Mgoogle/protobuf/timestamp.proto=github.com/gogo/protobuf/types,Mgoogle/protobuf/wrappers.proto=github.com/gogo/protobuf/types,Mgoogle/rpc/status.proto=github.com/gogo/googleapis/google/rpc\
:$GOPATH/src \
$GOPATH/src/github.com/kata-containers/kata-containers/src/agent/protocols/protos/$1"

    echo $cmd
    $cmd
    [ $? -eq 0 ] || die "Failed to generate golang file from $1"
}

generate_rust_sources() {
    local cmd="protoc --rust_out=./protocols/src/ \
--ttrpc_out=./protocols/src/,plugins=ttrpc:./protocols/src/ \
--plugin=protoc-gen-ttrpc=`which ttrpc_rust_plugin` \
-I $GOPATH/src/github.com/kata-containers/agent/vendor/github.com/gogo/protobuf:$GOPATH/src/github.com/kata-containers/agent/vendor:$GOPATH/src/github.com/gogo/protobuf:$GOPATH/src/github.com/gogo/googleapis:$GOPATH/src:$GOPATH/src/github.com/kata-containers/kata-containers/src/agent/protocols/protos \
$GOPATH/src/github.com/kata-containers/kata-containers/src/agent/protocols/protos/$1"

    echo $cmd
    $cmd
    [ $? -eq 0 ] || die "Failed to generate rust file from $1"

    if [ "$1" = "oci.proto" ]; then
        # Need change Box<Self> to ::std::boxed::Box<Self> because there is another struct Box
        sed 's/fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {/fn into_any(self: ::std::boxed::Box<Self>) -> ::std::boxed::Box<::std::any::Any> {/g' ./protocols/src/oci.rs > ./protocols/src/new_oci.rs
        sed 's/fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {/fn into_any(self: ::std::boxed::Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {/g' ./protocols/src/oci.rs > ./protocols/src/new_oci.rs
        mv ./protocols/src/new_oci.rs ./protocols/src/oci.rs
    fi;
}

if [ "$(basename $(pwd))" != "agent" ]; then
	die "Please go to directory of protocols before execute this shell"
fi

# Protocol buffer files required to generate golang/rust bindings.
proto_files_list=(agent.proto health.proto oci.proto github.com/kata-containers/agent/pkg/types/types.proto)

if [ "$1" = "" ]; then
    show_usage "${proto_files_list[@]}"
    exit 1
fi;

# pre-requirement check
which protoc
[ $? -eq 0 ] || die "Please install protoc from github.com/protocolbuffers/protobuf"

which protoc-gen-rust
[ $? -eq 0 ] || die "Please install protobuf-codegen from github.com/pingcap/grpc-rs"

which ttrpc_rust_plugin
[ $? -eq 0 ] || die "Please install ttrpc_rust_plugin from https://github.com/containerd/ttrpc-rust"

which protoc-gen-gogottrpc
[ $? -eq 0 ] || die "Please install protoc-gen-gogottrpc from https://github.com/containerd/ttrpc"

# do generate work
target=$1

# compile all proto files
if [ "$target" = "all" ]; then
    # compile all proto files
    for f in ${proto_files_list[@]}; do
        echo -e "\n   [golang] compiling ${f} ..."
        generate_go_sources $f
        echo -e "   [golang] ${f} compiled\n"

        echo -e "\n   [rust] compiling ${f} ..."
        generate_rust_sources $f
        echo -e "   [rust] ${f} compiled\n"
    done
else
    # compile individual proto file
    for f in ${proto_files_list[@]}; do
        if [ "$target" = "$f" ]; then
            echo -e "\n   [golang] compiling ${target} ..."
            generate_go_sources $target
            echo -e "   [golang] ${target} compiled\n"

            echo -e "\n   [rust] compiling ${target} ..."
            generate_rust_sources $target
            echo -e "   [rust] ${target} compiled\n"
        fi
    done
fi;

# if have no errors, compilation will succeed
show_succeed_msg
