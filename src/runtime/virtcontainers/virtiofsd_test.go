// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io"
	"io/ioutil"
	"os"
	"os/exec"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestVirtiofsdStart(t *testing.T) {
	assert := assert.New(t)
	type fields struct {
		path       string
		socketPath string
		cache      string
		extraArgs  []string
		sourcePath string
		debug      bool
		PID        int
		ctx        context.Context
	}

	sourcePath, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(sourcePath)

	socketDir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(socketDir)

	socketPath := socketDir + "socket.s"

	validConfig := fields{
		path:       "/usr/bin/virtiofsd-path",
		socketPath: socketPath,
		sourcePath: sourcePath,
	}
	NoDirectorySocket := validConfig
	NoDirectorySocket.socketPath = "/tmp/path/to/virtiofsd/socket.sock"

	tests := []struct {
		name    string
		fields  fields
		wantErr bool
	}{
		{"empty config", fields{}, true},
		{"Directory socket does not exist", NoDirectorySocket, true},
		{"valid config", validConfig, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := &virtiofsd{
				path:       tt.fields.path,
				socketPath: tt.fields.socketPath,
				cache:      tt.fields.cache,
				extraArgs:  tt.fields.extraArgs,
				sourcePath: tt.fields.sourcePath,
				debug:      tt.fields.debug,
				PID:        tt.fields.PID,
				ctx:        tt.fields.ctx,
				//Mock  wait function
				wait: func(runningCmd *exec.Cmd, stderr io.ReadCloser, debug bool) error {
					return nil
				},
			}
			var ctx context.Context
			_, err := v.Start(ctx)
			if (err != nil) != tt.wantErr {
				t.Errorf("virtiofsd.Start() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
		})
	}
}
