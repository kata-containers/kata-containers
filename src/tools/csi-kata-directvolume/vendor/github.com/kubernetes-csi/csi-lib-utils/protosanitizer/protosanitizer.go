/*
Copyright 2018 The Kubernetes Authors.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

// Package protosanitizer supports logging of gRPC messages without
// accidentally revealing sensitive fields.
package protosanitizer

import (
	"encoding/json"
	"fmt"
	"reflect"
	"strings"

	"github.com/golang/protobuf/descriptor"
	"github.com/golang/protobuf/proto"
	protobufdescriptor "github.com/golang/protobuf/protoc-gen-go/descriptor"
)

// StripSecrets returns a wrapper around the original CSI gRPC message
// which has a Stringer implementation that serializes the message
// as one-line JSON, but without including secret information.
// Instead of the secret value(s), the string "***stripped***" is
// included in the result.
//
// StripSecrets relies on an extension in CSI 1.0 and thus can only
// be used for messages based on that or a more recent spec!
//
// StripSecrets itself is fast and therefore it is cheap to pass the
// result to logging functions which may or may not end up serializing
// the parameter depending on the current log level.
func StripSecrets(msg interface{}) fmt.Stringer {
	return &stripSecrets{msg, isCSI1Secret}
}

// StripSecretsCSI03 is like StripSecrets, except that it works
// for messages based on CSI 0.3 and older. It does not work
// for CSI 1.0, use StripSecrets for that.
func StripSecretsCSI03(msg interface{}) fmt.Stringer {
	return &stripSecrets{msg, isCSI03Secret}
}

type stripSecrets struct {
	msg interface{}

	isSecretField func(field *protobufdescriptor.FieldDescriptorProto) bool
}

func (s *stripSecrets) String() string {
	// First convert to a generic representation. That's less efficient
	// than using reflect directly, but easier to work with.
	var parsed interface{}
	b, err := json.Marshal(s.msg)
	if err != nil {
		return fmt.Sprintf("<<json.Marshal %T: %s>>", s.msg, err)
	}
	if err := json.Unmarshal(b, &parsed); err != nil {
		return fmt.Sprintf("<<json.Unmarshal %T: %s>>", s.msg, err)
	}

	// Now remove secrets from the generic representation of the message.
	s.strip(parsed, s.msg)

	// Re-encoded the stripped representation and return that.
	b, err = json.Marshal(parsed)
	if err != nil {
		return fmt.Sprintf("<<json.Marshal %T: %s>>", s.msg, err)
	}
	return string(b)
}

func (s *stripSecrets) strip(parsed interface{}, msg interface{}) {
	protobufMsg, ok := msg.(descriptor.Message)
	if !ok {
		// Not a protobuf message, so we are done.
		return
	}

	// The corresponding map in the parsed JSON representation.
	parsedFields, ok := parsed.(map[string]interface{})
	if !ok {
		// Probably nil.
		return
	}

	// Walk through all fields and replace those with ***stripped*** that
	// are marked as secret. This relies on protobuf adding "json:" tags
	// on each field where the name matches the field name in the protobuf
	// spec (like volume_capabilities). The field.GetJsonName() method returns
	// a different name (volumeCapabilities) which we don't use.
	_, md := descriptor.ForMessage(protobufMsg)
	fields := md.GetField()
	if fields != nil {
		for _, field := range fields {
			if s.isSecretField(field) {
				// Overwrite only if already set.
				if _, ok := parsedFields[field.GetName()]; ok {
					parsedFields[field.GetName()] = "***stripped***"
				}
			} else if field.GetType() == protobufdescriptor.FieldDescriptorProto_TYPE_MESSAGE {
				// When we get here,
				// the type name is something like ".csi.v1.CapacityRange" (leading dot!)
				// and looking up "csi.v1.CapacityRange"
				// returns the type of a pointer to a pointer
				// to CapacityRange. We need a pointer to such
				// a value for recursive stripping.
				typeName := field.GetTypeName()
				if strings.HasPrefix(typeName, ".") {
					typeName = typeName[1:]
				}
				t := proto.MessageType(typeName)
				if t == nil || t.Kind() != reflect.Ptr {
					// Shouldn't happen, but
					// better check anyway instead
					// of panicking.
					continue
				}
				v := reflect.New(t.Elem())

				// Recursively strip the message(s) that
				// the field contains.
				i := v.Interface()
				entry := parsedFields[field.GetName()]
				if slice, ok := entry.([]interface{}); ok {
					// Array of values, like VolumeCapabilities in CreateVolumeRequest.
					for _, entry := range slice {
						s.strip(entry, i)
					}
				} else {
					// Single value.
					s.strip(entry, i)
				}
			}
		}
	}
}

// isCSI1Secret uses the csi.E_CsiSecret extension from CSI 1.0 to
// determine whether a field contains secrets.
func isCSI1Secret(field *protobufdescriptor.FieldDescriptorProto) bool {
	ex, err := proto.GetExtension(field.Options, e_CsiSecret)
	return err == nil && ex != nil && *ex.(*bool)
}

// Copied from the CSI 1.0 spec (https://github.com/container-storage-interface/spec/blob/37e74064635d27c8e33537c863b37ccb1182d4f8/lib/go/csi/csi.pb.go#L4520-L4527)
// to avoid a package dependency that would prevent usage of this package
// in repos using an older version of the spec.
//
// Future revision of the CSI spec must not change this extensions, otherwise
// they will break filtering in binaries based on the 1.0 version of the spec.
var e_CsiSecret = &proto.ExtensionDesc{
	ExtendedType:  (*protobufdescriptor.FieldOptions)(nil),
	ExtensionType: (*bool)(nil),
	Field:         1059,
	Name:          "csi.v1.csi_secret",
	Tag:           "varint,1059,opt,name=csi_secret,json=csiSecret",
	Filename:      "github.com/container-storage-interface/spec/csi.proto",
}

// isCSI03Secret relies on the naming convention in CSI <= 0.3
// to determine whether a field contains secrets.
func isCSI03Secret(field *protobufdescriptor.FieldDescriptorProto) bool {
	return strings.HasSuffix(field.GetName(), "_secrets")
}
