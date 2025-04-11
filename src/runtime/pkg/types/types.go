// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package types

const (
	DefaultKataRuntimeName    = "io.containerd.kata.v2"
	KataRuntimeNameRegexp     = `io\.containerd\.kata.*\.v2`
	ContainerdRuntimeTaskPath = "io.containerd.runtime.v2.task"
)

type Initdata struct {
	Version   string            `toml:"version"`
	Algorithm string            `toml:"algorithm"`
	Data      map[string]string `toml:"data"`
}
