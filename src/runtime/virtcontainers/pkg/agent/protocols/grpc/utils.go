//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package grpc

import (
	"reflect"

	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"

	"github.com/opencontainers/runtime-spec/specs-go"
)

func copyValue(to, from reflect.Value) error {
	toKind := to.Kind()
	fromKind := from.Kind()

	if !from.IsValid() {
		return nil
	}

	if toKind == reflect.Ptr {
		// Handle the case of nil pointers.
		if fromKind == reflect.Ptr && from.IsNil() {
			return nil
		}

		// If the destination and the origin are both pointers, and
		// if the origin is not nil, we need to allocate a new one
		// for the destination.
		to.Set(reflect.New(to.Type().Elem()))
		if fromKind == reflect.Ptr {
			return copyValue(to.Elem(), from.Elem())
		}

		return copyValue(to.Elem(), from)
	}

	// Here the destination is not a pointer.
	// Let's check what's the origin.
	if fromKind == reflect.Ptr {
		return copyValue(to, from.Elem())
	}

	switch toKind {
	case reflect.Struct:
		return copyStructValue(to, from)
	case reflect.Slice:
		return copySliceValue(to, from)
	case reflect.Map:
		return copyMapValue(to, from)
	case reflect.Interface:
		if to.Type().Name() == "isLinuxSyscall_ErrnoRet" {
			dest := LinuxSyscall_Errnoret{Errnoret: uint32(from.Uint())}
			var destintf isLinuxSyscall_ErrnoRet = &dest
			toVal := reflect.ValueOf(destintf)
			to.Set(toVal)
			return nil
		}
		return grpcStatus.Errorf(codes.InvalidArgument, "Can not convert %v to %v, kind= %v", from.Type(), to.Type(), toKind)
	default:
		// We now are copying non pointers scalar.
		// This is the leaf of the recursion.
		if from.Type() != to.Type() {
			if from.Type().ConvertibleTo(to.Type()) {
				to.Set(from.Convert(to.Type()))
				return nil
			}

			return grpcStatus.Errorf(codes.InvalidArgument, "Can not convert %v to %v, kind= %v", from.Type(), to.Type(), toKind)
		}

		to.Set(from)
		return nil
	}
}

func copyMapValue(to, from reflect.Value) error {
	if to.Kind() != reflect.Map && from.Kind() != reflect.Map {
		return grpcStatus.Errorf(codes.InvalidArgument, "Can only copy maps into maps")
	}

	to.Set(reflect.MakeMap(to.Type()))

	keys := from.MapKeys()

	for _, k := range keys {
		newValue := reflect.New(to.Type().Elem())
		v := from.MapIndex(k)

		if err := copyValue(newValue.Elem(), v); err != nil {
			return err
		}

		to.SetMapIndex(k, newValue.Elem())
	}

	return nil
}

func copySliceValue(to, from reflect.Value) error {
	if to.Kind() != reflect.Slice && from.Kind() != reflect.Slice {
		return grpcStatus.Errorf(codes.InvalidArgument, "Can only copy slices into slices")
	}

	sliceLen := from.Len()
	to.Set(reflect.MakeSlice(to.Type(), sliceLen, sliceLen))

	for j := 0; j < sliceLen; j++ {
		if err := copyValue(to.Index(j), from.Index(j)); err != nil {
			return err
		}
	}

	return nil
}

func copyStructSkipField(to, from reflect.Value) bool {
	var grpcSolaris Solaris
	var ociSolaris specs.Solaris
	var grpcWindows Windows
	var ociWindows specs.Windows

	toType := to.Type()
	grpcSolarisType := reflect.TypeOf(&grpcSolaris)
	ociSolarisType := reflect.TypeOf(ociSolaris)
	grpcWindowsType := reflect.TypeOf(&grpcWindows)
	ociWindowsType := reflect.TypeOf(ociWindows)

	// We skip all Windows and Solaris types
	if toType == grpcSolarisType || toType == grpcWindowsType || toType == ociSolarisType || toType == ociWindowsType {
		return true
	}

	return false
}

func structFieldName(v reflect.Value, index int) (string, error) {
	if v.Kind() != reflect.Struct {
		return "", grpcStatus.Errorf(codes.InvalidArgument, "Can only infer field name from structs")
	}

	return v.Type().Field(index).Name, nil
}

func isEmbeddedStruct(v reflect.Value, index int) bool {
	if v.Kind() != reflect.Struct || index > v.Type().NumField()-1 {
		return false
	}

	return v.Type().Field(index).Anonymous
}

func findStructField(v reflect.Value, name string) (reflect.Value, error) {
	if v.Kind() != reflect.Struct {
		return reflect.Value{}, grpcStatus.Errorf(codes.InvalidArgument, "Can only infer field name from structs")
	}

	for i := 0; i < v.NumField(); i++ {
		if v.Type().Field(i).Name == name {
			return v.Field(i), nil
		}
	}

	return reflect.Value{}, grpcStatus.Errorf(codes.InvalidArgument, "Could not find field %s", name)
}

func copyStructValue(to, from reflect.Value) error {
	if to.Kind() != reflect.Struct && from.Kind() != reflect.Struct {
		return grpcStatus.Errorf(codes.InvalidArgument, "Can only copy structs into structs")
	}

	if copyStructSkipField(to, from) {
		return nil
	}

	for i := 0; i < to.NumField(); i++ {
		// If one of the field is embedded, we copy between the embedded field
		// and the structure itself. The fields in the embedded field should
		// be found in the parent structure.
		if isEmbeddedStruct(to, i) {
			if err := copyStructValue(to.Field(i), from); err != nil {
				return err
			}
			continue
		}

		if isEmbeddedStruct(from, i) {
			if err := copyStructValue(to, from.Field(i)); err != nil {
				return err
			}
			continue
		}

		// Find the destination structure field name.
		fieldName, err := structFieldName(to, i)
		if err != nil {
			return err
		}

		// Try to find the same field name in the origin structure.
		// This can fail as we support copying between structures
		// that optionally have embedded fields.
		v, err := findStructField(from, fieldName)
		if err != nil {
			continue
		}

		if err := copyValue(to.Field(i), v); err != nil {
			return err
		}
	}

	return nil
}

func copyStruct(to interface{}, from interface{}) (err error) {
	defer func() {
		if r := recover(); r != nil {
			err = r.(error)
		}
	}()

	toVal := reflect.ValueOf(to)
	fromVal := reflect.ValueOf(from)

	if toVal.Kind() != reflect.Ptr || toVal.Elem().Kind() != reflect.Struct ||
		fromVal.Kind() != reflect.Ptr || fromVal.Elem().Kind() != reflect.Struct {
		return grpcStatus.Errorf(codes.InvalidArgument, "Arguments must be pointers to structures")
	}

	toVal = toVal.Elem()
	fromVal = fromVal.Elem()

	return copyStructValue(toVal, fromVal)
}

// OCItoGRPC converts an OCI specification to its gRPC representation
func OCItoGRPC(ociSpec *specs.Spec) (*Spec, error) {
	s := &Spec{}

	err := copyStruct(s, ociSpec)

	return s, err
}

// ProcessOCItoGRPC converts an OCI process specification into its gRPC
// representation
func ProcessOCItoGRPC(ociProcess *specs.Process) (*Process, error) {
	s := &Process{}

	err := copyStruct(s, ociProcess)

	return s, err
}

// ProcessGRPCtoOCI converts a gRPC specification back into an OCI
// representation
func ProcessGRPCtoOCI(grpcProcess *Process) (*specs.Process, error) {
	s := &specs.Process{}

	err := copyStruct(s, grpcProcess)

	return s, err
}

// ResourcesOCItoGRPC converts an OCI LinuxResources specification into its gRPC
// representation
func ResourcesOCItoGRPC(ociResources *specs.LinuxResources) (*LinuxResources, error) {
	s := &LinuxResources{}

	err := copyStruct(s, ociResources)

	return s, err
}

// ResourcesGRPCtoOCI converts an gRPC LinuxResources specification into its OCI
// representation
func ResourcesGRPCtoOCI(grpcResources *LinuxResources) (*specs.LinuxResources, error) {
	s := &specs.LinuxResources{}

	err := copyStruct(s, grpcResources)

	return s, err
}
