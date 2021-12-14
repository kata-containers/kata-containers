// Copyright (c) 2019 Hyper.sh
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"os"
	"path/filepath"
	"strconv"

	"github.com/pkg/errors"
	"github.com/prometheus/procfs"
)

const taskPath = "task"

type Proc struct {
	*procfs.Proc
}

func NewProc(pid int) (*Proc, error) {
	p, err := procfs.NewProc(pid)
	if err != nil {
		return nil, errors.Wrapf(err, "Invalid pid %v", pid)
	}

	return &Proc{&p}, nil
}

// We should try to upstream this but let's keep it until upstream supports it.
func (p *Proc) Children() ([]*Proc, error) {
	parent := strconv.Itoa(p.PID)
	infos, err := os.ReadDir(filepath.Join(procfs.DefaultMountPoint, parent, taskPath))
	if err != nil {
		return nil, errors.Wrapf(err, "Fail to read pid %v proc task dir", p.PID)
	}

	var children []*Proc
	for _, info := range infos {
		if !info.IsDir() || info.Name() == parent {
			continue
		}
		pid, err := strconv.Atoi(info.Name())
		if err != nil {
			return nil, errors.Wrapf(err, "Invalid child pid %v", info.Name())
		}
		child, err := NewProc(pid)
		if err != nil {
			return nil, err
		}
		children = append(children, child)
	}

	return children, nil
}
