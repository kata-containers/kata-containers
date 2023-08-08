// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package compatoci

import (
	"encoding/json"
	"os"
	"path"
	"path/filepath"
	"testing"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const (
	tempBundlePath        = "/tmp/virtc/ocibundle/"
	capabilitiesSpecArray = `
		{
		    "ociVersion": "1.0.0-rc2-dev",
		    "process": {
		        "capabilities": [
		            "CAP_CHOWN",
		            "CAP_DAC_OVERRIDE",
		            "CAP_FSETID"
		        ]
		    }
		}`

	capabilitiesSpecStruct = `
		{
		    "ociVersion": "1.0.0-rc5",
		    "process": {
		        "capabilities": {
		            "bounding": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ],
		            "effective": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ],
		            "inheritable": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ],
		            "permitted": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ]
		        }
		    }
		}`
)

func TestContainerCapabilities(t *testing.T) {
	var ociSpec compatOCISpec

	ociSpec.Process = &compatOCIProcess{}
	ociSpec.Process.Capabilities = map[string]interface{}{
		"bounding":    []interface{}{"CAP_KILL"},
		"effective":   []interface{}{"CAP_KILL", "CAP_LEASE"},
		"permitted":   []interface{}{"CAP_SETUID"},
		"inheritable": []interface{}{"CAP_KILL", "CAP_LEASE", "CAP_SYS_ADMIN"},
		"ambient":     []interface{}{""},
	}

	c, err := ContainerCapabilities(ociSpec)
	assert.Nil(t, err)
	assert.Equal(t, c.Bounding, []string{"CAP_KILL"})
	assert.Equal(t, c.Effective, []string{"CAP_KILL", "CAP_LEASE"})
	assert.Equal(t, c.Permitted, []string{"CAP_SETUID"})
	assert.Equal(t, c.Inheritable, []string{"CAP_KILL", "CAP_LEASE", "CAP_SYS_ADMIN"})
	assert.Equal(t, c.Ambient, []string{""})

	ociSpec.Process.Capabilities = map[string]interface{}{
		"unexpected": interface{}(""),
	}

	c, err = ContainerCapabilities(ociSpec)
	assert.NotNil(t, err)
	assert.Equal(t, len(c.Bounding), 0)
	assert.Equal(t, len(c.Effective), 0)
	assert.Equal(t, len(c.Permitted), 0)
	assert.Equal(t, len(c.Inheritable), 0)
	assert.Equal(t, len(c.Ambient), 0)

	ociSpec.Process.Capabilities = []interface{}{"CAP_LEASE", "CAP_SETUID"}

	c, err = ContainerCapabilities(ociSpec)
	assert.Nil(t, err)
	assert.Equal(t, c.Bounding, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Effective, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Permitted, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Inheritable, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Ambient, []string{"CAP_LEASE", "CAP_SETUID"})

	ociSpec.Process.Capabilities = nil

	c, err = ContainerCapabilities(ociSpec)
	assert.Nil(t, err)
	assert.Equal(t, len(c.Bounding), 0)
	assert.Equal(t, len(c.Effective), 0)
	assert.Equal(t, len(c.Permitted), 0)
	assert.Equal(t, len(c.Inheritable), 0)
	assert.Equal(t, len(c.Ambient), 0)

	ociSpec.Process.Capabilities = interface{}("")

	c, err = ContainerCapabilities(ociSpec)
	assert.NotNil(t, err)
	assert.Equal(t, len(c.Bounding), 0)
	assert.Equal(t, len(c.Effective), 0)
	assert.Equal(t, len(c.Permitted), 0)
	assert.Equal(t, len(c.Inheritable), 0)
	assert.Equal(t, len(c.Ambient), 0)

	ociSpec.Process = nil

	c, err = ContainerCapabilities(ociSpec)
	assert.NotNil(t, err)
	assert.Equal(t, len(c.Bounding), 0)
	assert.Equal(t, len(c.Effective), 0)
	assert.Equal(t, len(c.Permitted), 0)
	assert.Equal(t, len(c.Inheritable), 0)
	assert.Equal(t, len(c.Ambient), 0)
}

// use specs.Spec to decode the spec, the content of capabilities is [] string
func TestCompatOCISpecWithArray(t *testing.T) {
	compatOCISpec := compatOCISpec{}
	err := json.Unmarshal([]byte(capabilitiesSpecArray), &compatOCISpec)
	assert.Nil(t, err, "use compatOCISpec to decode capabilitiesSpecArray failed")

	ociSpecJSON, err := json.Marshal(compatOCISpec)
	assert.Nil(t, err, "encode compatOCISpec failed")

	// use specs.Spec to decode the spec, specs.Spec' capabilities is struct,
	// but the content of spec' capabilities is [] string
	ociSpec := specs.Spec{}
	err = json.Unmarshal(ociSpecJSON, &ociSpec)
	assert.NotNil(t, err, "This test should fail")

	caps, err := ContainerCapabilities(compatOCISpec)
	assert.Nil(t, err, "decode capabilities failed")
	compatOCISpec.Process.Capabilities = caps

	ociSpecJSON, err = json.Marshal(compatOCISpec)
	assert.Nil(t, err, "encode compatOCISpec failed")

	// capabilities has been chaged to struct
	err = json.Unmarshal(ociSpecJSON, &ociSpec)
	assert.Nil(t, err, "This test should fail")
}

// use specs.Spec to decode the spec, the content of capabilities is struct
func TestCompatOCISpecWithStruct(t *testing.T) {
	compatOCISpec := compatOCISpec{}
	err := json.Unmarshal([]byte(capabilitiesSpecStruct), &compatOCISpec)
	assert.Nil(t, err, "use compatOCISpec to decode capabilitiesSpecStruct failed")

	ociSpecJSON, err := json.Marshal(compatOCISpec)
	assert.Nil(t, err, "encode compatOCISpec failed")

	ociSpec := specs.Spec{}
	err = json.Unmarshal(ociSpecJSON, &ociSpec)
	assert.Nil(t, err, "This test should not fail")
}

func TestGetConfigPath(t *testing.T) {
	expected := filepath.Join(tempBundlePath, "config.json")
	configPath := getConfigPath(tempBundlePath)
	assert.Equal(t, configPath, expected)
}

func TestParseConfigJSON(t *testing.T) {
	tmpDir := t.TempDir()

	var ociSpec compatOCISpec
	var configByte []byte

	ociSpec.Spec.Version = "1.0.0"
	ociSpec.Process = &compatOCIProcess{}
	ociSpec.Process.Capabilities = map[string]interface{}{
		"bounding":    []interface{}{"CAP_KILL"},
		"effective":   []interface{}{"CAP_KILL", "CAP_LEASE"},
		"permitted":   []interface{}{"CAP_SETUID"},
		"inheritable": []interface{}{"CAP_KILL", "CAP_LEASE", "CAP_SYS_ADMIN"},
		"ambient":     []interface{}{""},
	}

	file, err := os.Create(path.Join(tmpDir, "config.json"))
	if err != nil {
		t.Error("cannot create file")
	}
	defer file.Close()

	if configByte, err = json.Marshal(ociSpec); err != nil {
		t.Error("cannot marshal compat oci spec")
	}

	_, err = file.Write(configByte)
	if err != nil {
		t.Error("cannot write config data into file")
	}

	spec, err := ParseConfigJSON(tmpDir)
	assert.Nil(t, err)
	assert.Equal(t, spec.Version, "1.0.0")
}

func TestGetContainerSpec(t *testing.T) {
	annotations := map[string]string{
		"io.katacontainers.pkg.oci.bundle_path": "",
	}

	_, err := GetContainerSpec(annotations)
	assert.NotNil(t, err)

	annotations = map[string]string{
		"io.katacontainers.pkg.oci.wrong_path": "",
	}

	_, err = GetContainerSpec(annotations)
	assert.NotNil(t, err)
}
