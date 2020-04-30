// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package compatoci

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"path/filepath"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"

	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
)

var ociLog = logrus.WithFields(logrus.Fields{
	"source":    "virtcontainers",
	"subsystem": "compatoci",
})

// compatOCIProcess is a structure inheriting from specs.Process defined
// in runtime-spec/specs-go package. The goal is to be compatible with
// both v1.0.0-rc4 and v1.0.0-rc5 since the latter introduced a change
// about the type of the Capabilities field.
// Refer to: https://github.com/opencontainers/runtime-spec/commit/37391fb
type compatOCIProcess struct {
	specs.Process
	Capabilities interface{} `json:"capabilities,omitempty" platform:"linux"` //nolint:govet
}

// compatOCISpec is a structure inheriting from specs.Spec defined
// in runtime-spec/specs-go package. It relies on the compatOCIProcess
// structure declared above, in order to be compatible with both
// v1.0.0-rc4 and v1.0.0-rc5.
// Refer to: https://github.com/opencontainers/runtime-spec/commit/37391fb
type compatOCISpec struct {
	specs.Spec
	Process *compatOCIProcess `json:"process,omitempty"` //nolint:govet
}

func containerCapabilities(s compatOCISpec) (specs.LinuxCapabilities, error) {
	capabilities := s.Process.Capabilities
	var c specs.LinuxCapabilities

	// In spec v1.0.0-rc4, capabilities was a list of strings. This was changed
	// to an object with v1.0.0-rc5.
	// Check for the interface type to support both the versions.
	switch caps := capabilities.(type) {
	case map[string]interface{}:
		for key, value := range caps {
			switch val := value.(type) {
			case []interface{}:
				var list []string

				for _, str := range val {
					list = append(list, str.(string))
				}

				switch key {
				case "bounding":
					c.Bounding = list
				case "effective":
					c.Effective = list
				case "inheritable":
					c.Inheritable = list
				case "ambient":
					c.Ambient = list
				case "permitted":
					c.Permitted = list
				}

			default:
				return c, fmt.Errorf("Unexpected format for capabilities: %v", caps)
			}
		}
	case []interface{}:
		var list []string
		for _, str := range caps {
			list = append(list, str.(string))
		}

		c = specs.LinuxCapabilities{
			Bounding:    list,
			Effective:   list,
			Inheritable: list,
			Ambient:     list,
			Permitted:   list,
		}
	case nil:
		ociLog.Debug("Empty capabilities have been passed")
		return c, nil
	default:
		return c, fmt.Errorf("Unexpected format for capabilities: %v", caps)
	}

	return c, nil
}

// SetLogger sets up a logger for this pkg
func SetLogger(logger *logrus.Entry) {
	fields := ociLog.Data

	ociLog = logger.WithFields(fields)
}

// ContainerCapabilities return a LinuxCapabilities for virtcontainer
func ContainerCapabilities(s compatOCISpec) (specs.LinuxCapabilities, error) {
	if s.Process == nil {
		return specs.LinuxCapabilities{}, fmt.Errorf("ContainerCapabilities, Process is nil")
	}
	return containerCapabilities(s)
}

// getConfigPath returns the full config path from the bundle
// path provided.
func getConfigPath(bundlePath string) string {
	return filepath.Join(bundlePath, "config.json")
}

// ParseConfigJSON unmarshals the config.json file.
func ParseConfigJSON(bundlePath string) (specs.Spec, error) {
	configPath := getConfigPath(bundlePath)
	ociLog.Debugf("converting %s", configPath)

	configByte, err := ioutil.ReadFile(configPath)
	if err != nil {
		return specs.Spec{}, err
	}

	var compSpec compatOCISpec
	if err := json.Unmarshal(configByte, &compSpec); err != nil {
		return specs.Spec{}, err
	}

	caps, err := ContainerCapabilities(compSpec)
	if err != nil {
		return specs.Spec{}, err
	}

	compSpec.Spec.Process = &compSpec.Process.Process
	compSpec.Spec.Process.Capabilities = &caps

	return compSpec.Spec, nil
}

func GetContainerSpec(annotations map[string]string) (specs.Spec, error) {
	if bundlePath, ok := annotations[vcAnnotations.BundlePathKey]; ok {
		return ParseConfigJSON(bundlePath)
	}

	ociLog.Errorf("Annotations[%s] not found, cannot find container spec",
		vcAnnotations.BundlePathKey)
	return specs.Spec{}, fmt.Errorf("Could not find container spec")
}
