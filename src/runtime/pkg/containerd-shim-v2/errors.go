// Copyright (c) 2019 hyper.sh
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"strings"
	"syscall"

	"github.com/pkg/errors"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

// toGRPC maps the virtcontainers error into a grpc error,
// using the original error message as a description.
func toGRPC(err error) error {
	if err == nil {
		return nil
	}

	if isGRPCError(err) {
		// error has already been mapped to grpc
		return err
	}

	err = errors.Cause(err)
	switch {
	case isInvalidArgument(err):
		return status.Errorf(codes.InvalidArgument, err.Error())
	case isNotFound(err):
		return status.Errorf(codes.NotFound, err.Error())
	}

	return err
}

// toGRPCf maps the error to grpc error codes, assembling the formatting string
// and combining it with the target error string.
func toGRPCf(err error, format string, args ...interface{}) error {
	return toGRPC(errors.Wrapf(err, format, args...))
}

func isGRPCError(err error) bool {
	_, ok := status.FromError(err)
	return ok
}

func isInvalidArgument(err error) bool {
	return err == vc.ErrNeedSandbox || err == vc.ErrNeedSandboxID ||
		err == vc.ErrNeedContainerID || err == vc.ErrNeedState ||
		err == syscall.EINVAL
}

func isNotFound(err error) bool {
	return err == vc.ErrNoSuchContainer || err == syscall.ENOENT ||
		strings.Contains(err.Error(), "not found") || strings.Contains(err.Error(), "not exist")
}

func isGRPCErrorCode(code codes.Code, err error) bool {
	s, ok := status.FromError(err)
	if !ok {
		return false
	}
	return s != nil && s.Code() == code
}
