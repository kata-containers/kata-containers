#
# Copyright 2019 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

CACHE_PATH="protocols/cache"

protoc \
    -I=$GOPATH/src \
    --proto_path=$CACHE_PATH \
    --go_out=$CACHE_PATH \
    --go-grpc_out=$CACHE_PATH \
    $CACHE_PATH/cache.proto
