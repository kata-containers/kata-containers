#!/bin/bash

die() {
    echo $1
    exit
}

get_source_version() {
    if [ ! -d $GOPATH/src/$1 ]; then
        go get -d -v $1
    fi
    [ $? -eq 0 ] || die "Failed to get $1"
    if [ "$2" != "" ] ; then
	    pushd "${GOPATH}/src/$1"
        if [ $(git rev-parse HEAD) != $2 ] ; then
            git checkout $2
            [ $? -eq 0 ] || die "Failed to get $1 $2"
        fi
        popd
    fi
}

get_rs() {
    local cmd="protoc --rust_out=./src/ --grpc_out=./src/,plugins=grpc:./src/ --plugin=protoc-gen-grpc=`which grpc_rust_plugin` -I ./protos/ ./protos/$1"
    echo $cmd
    $cmd
    [ $? -eq 0 ] || die "Failed to get rust from $1"
}

if [ "$(basename $(pwd))" != "protocols" ] || [ ! -d "./hack/" ]; then
	die "Please go to directory of protocols before execute this shell"
fi
which protoc
[ $? -eq 0 ] || die "Please install protoc from github.com/protocolbuffers/protobuf"
which protoc-gen-rust
[ $? -eq 0 ] || die "Please install protobuf-codegen from github.com/pingcap/grpc-rs"
which grpc_rust_plugin
[ $? -eq 0 ] || die "Please install grpc_rust_plugin from github.com/pingcap/grpc-rs"

if [ $UPDATE_PROTOS ]; then
    if [ ! $GOPATH ]; then
        die 'Need $GOPATH to get the proto files'
    fi

    get_source_version "github.com/kata-containers/agent" ""
    cp $GOPATH/src/github.com/kata-containers/agent/protocols/grpc/agent.proto ./protos/
    cp $GOPATH/src/github.com/kata-containers/agent/protocols/grpc/oci.proto ./protos/
    cp $GOPATH/src/github.com/kata-containers/agent/protocols/grpc/health.proto ./protos/
    mkdir -p ./protos/github.com/kata-containers/agent/pkg/types/
    cp $GOPATH/src/github.com/kata-containers/agent/pkg/types/types.proto ./protos/github.com/kata-containers/agent/pkg/types/

    # The version is get from https://github.com/kata-containers/agent/blob/master/Gopkg.toml
    get_source_version "github.com/gogo/protobuf" "4cbf7e384e768b4e01799441fdf2a706a5635ae7"
    mkdir -p ./protos/github.com/gogo/protobuf/gogoproto/
    cp $GOPATH/src/github.com/gogo/protobuf/gogoproto/gogo.proto ./protos/github.com/gogo/protobuf/gogoproto/
    mkdir -p ./protos/google/protobuf/
    cp $GOPATH/src/github.com/gogo/protobuf/protobuf/google/protobuf/empty.proto ./protos/google/protobuf/
fi

get_rs agent.proto
get_rs health.proto
get_rs github.com/kata-containers/agent/pkg/types/types.proto
get_rs google/protobuf/empty.proto

get_rs oci.proto
# Need change Box<Self> to ::std::boxed::Box<Self> because there is another struct Box
sed 's/fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {/fn into_any(self: ::std::boxed::Box<Self>) -> ::std::boxed::Box<::std::any::Any> {/g' src/oci.rs > src/new_oci.rs
mv src/new_oci.rs src/oci.rs
