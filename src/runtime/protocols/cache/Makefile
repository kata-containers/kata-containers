# Copyright (c) 2021 Easystack Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

.PHONY: gen

gen:
	protoc \
	-I=$$GOPATH/src \
	-I=$$GOPATH/src/github.com/gogo/protobuf/protobuf \
	--proto_path=. \
	--gogofast_out=Mgoogle/protobuf/empty.proto=github.com/gogo/protobuf/types,plugins=grpc:. \
	cache.proto
