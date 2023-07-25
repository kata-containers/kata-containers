// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
package k8s

import (
	"fmt"

	exec "github.com/kata-containers/tests/metrics/exec"
)

type execOpt struct {
	showInStdOut bool
}

type ExecOption func(e *execOpt)

func ExecOptShowStdOut() ExecOption {
	return func(e *execOpt) {
		e.showInStdOut = true
	}

}

func (p *Pod) Exec(cmd string, opts ...ExecOption) (output string, err error) {
	log.Debugf("Exec %q in %s", cmd, p.YamlPath)
	o := &execOpt{showInStdOut: false}
	for _, opt := range opts {
		opt(o)

	}
	execCmd := fmt.Sprintf("kubectl exec -f  %s -- /bin/bash -c %q", p.YamlPath, cmd)
	return exec.ExecCmd(execCmd, Debug || o.showInStdOut)
}
