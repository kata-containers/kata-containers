// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/errdefs"
	googleProtobuf "github.com/gogo/protobuf/types"
	vc "github.com/kata-containers/runtime/virtcontainers"
	specs "github.com/opencontainers/runtime-spec/specs-go"
)

type exec struct {
	container *container
	cmds      *vc.Cmd
	tty       *tty
	ttyio     *ttyIO
	id        string

	exitCode int32

	status task.Status

	exitIOch chan struct{}
	exitCh   chan uint32

	exitTime time.Time
}

type tty struct {
	stdin    string
	stdout   string
	stderr   string
	height   uint32
	width    uint32
	terminal bool
}

func getEnvs(envs []string) []vc.EnvVar {
	var vcEnvs = []vc.EnvVar{}
	var env vc.EnvVar

	for _, v := range envs {
		pair := strings.SplitN(v, "=", 2)

		if len(pair) == 2 {
			env = vc.EnvVar{Var: pair[0], Value: pair[1]}
		} else if len(pair) == 1 {
			env = vc.EnvVar{Var: pair[0], Value: ""}
		}

		vcEnvs = append(vcEnvs, env)
	}

	return vcEnvs
}

func newExec(c *container, stdin, stdout, stderr string, terminal bool, jspec *googleProtobuf.Any) (*exec, error) {
	var height uint32
	var width uint32

	if jspec == nil {
		return nil, errdefs.ToGRPCf(errdefs.ErrInvalidArgument, "googleProtobuf.Any points to nil")
	}

	// process exec request
	var spec specs.Process
	if err := json.Unmarshal(jspec.Value, &spec); err != nil {
		return nil, err
	}

	if spec.ConsoleSize != nil {
		height = uint32(spec.ConsoleSize.Height)
		width = uint32(spec.ConsoleSize.Width)
	}

	tty := &tty{
		stdin:    stdin,
		stdout:   stdout,
		stderr:   stderr,
		height:   height,
		width:    width,
		terminal: terminal,
	}

	cmds := &vc.Cmd{
		Args:            spec.Args,
		Envs:            getEnvs(spec.Env),
		User:            fmt.Sprintf("%d", spec.User.UID),
		PrimaryGroup:    fmt.Sprintf("%d", spec.User.GID),
		WorkDir:         spec.Cwd,
		Interactive:     terminal,
		Detach:          !terminal,
		NoNewPrivileges: spec.NoNewPrivileges,
	}

	exec := &exec{
		container: c,
		cmds:      cmds,
		tty:       tty,
		exitCode:  exitCode255,
		exitIOch:  make(chan struct{}),
		exitCh:    make(chan uint32, 1),
		status:    task.StatusCreated,
	}

	return exec, nil
}

func (c *container) getExec(id string) (*exec, error) {
	if c.execs == nil {
		return nil, errdefs.ToGRPCf(errdefs.ErrNotFound, "exec does not exist %s", id)
	}

	exec := c.execs[id]

	if exec == nil {
		return nil, errdefs.ToGRPCf(errdefs.ErrNotFound, "exec does not exist %s", id)
	}

	return exec, nil
}
