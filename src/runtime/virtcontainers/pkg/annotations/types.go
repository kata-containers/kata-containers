// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package annotations

import "encoding/json"

// types and structs to handle emptydir
// see https://kubernetes.io/docs/concepts/storage/volumes/#emptydir

const (
	// annoation key
	kataAnnotSandboxVolumesPrefix         = kataAnnotSandboxPrefix + "volumes."
	KataAnnotSandboxVolumesEmptyDirPrefix = kataAnnotSandboxVolumesPrefix + "emptydir"

	EmptyDirMediumMemory = "Memory"
)

type EmptyDirs struct {
	EmptyDirs []*EmptyDir
}

type EmptyDir struct {
	// attributes from volumes
	Name      string `json:"name"`
	Medium    string `json:"medium,omitempty"`
	SizeLimit string `json:"size_limit,omitempty"`
}

func (ed *EmptyDir) IsMemoryBackended() bool {
	return ed.Medium == EmptyDirMediumMemory
}

func ParseEmptyDirs(value string) (*EmptyDirs, error) {
	eds := &EmptyDirs{}
	err := json.Unmarshal([]byte(value), eds)
	if err != nil {
		return nil, err
	}

	return eds, nil
}

func (eds *EmptyDirs) String() (string, error) {
	data, err := json.Marshal(eds)
	if err != nil {
		return "", err
	}

	return string(data), nil
}
