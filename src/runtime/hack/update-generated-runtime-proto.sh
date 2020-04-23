#
# Copyright 2019 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

protoc \
	-I=$GOPATH/src \
	-I=$GOPATH/src/github.com/gogo/protobuf/protobuf \
	--proto_path=protocols/cache \
	--gogofast_out=\
Mgoogle/protobuf/empty.proto=github.com/gogo/protobuf/types,\
plugins=grpc:protocols/cache \
	protocols/cache/cache.proto
