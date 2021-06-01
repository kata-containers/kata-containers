// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io/ioutil"
	"os"
	"strings"
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
			}
			var ctx context.Context
			_, err := v.Start(ctx, nil)
			if (err != nil) != tt.wantErr {
				t.Errorf("virtiofsd.Start() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
		})
	}
}

func TestVirtiofsdArgs(t *testing.T) {
	assert := assert.New(t)

	v := &virtiofsd{
		path:       "/usr/bin/virtiofsd",
		sourcePath: "/run/kata-shared/foo",
		cache:      "none",
	}

	expected := "--syslog -o cache=none -o no_posix_lock -o source=/run/kata-shared/foo --fd=123 -f"
	args, err := v.args(123)
	assert.NoError(err)
	assert.Equal(expected, strings.Join(args, " "))

	v.debug = false
	expected = "--syslog -o cache=none -o no_posix_lock -o source=/run/kata-shared/foo --fd=456 -f"
	args, err = v.args(456)
	assert.NoError(err)
	assert.Equal(expected, strings.Join(args, " "))
}

func TestValid(t *testing.T) {
	assert := assert.New(t)

	sourcePath, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(sourcePath)

	socketDir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(socketDir)

	socketPath := socketDir + "socket.s"

	newVirtiofsdFunc := func() *virtiofsd {
		return &virtiofsd{
			path:       "/usr/bin/virtiofsd",
			sourcePath: sourcePath,
			socketPath: socketPath,
		}
	}

	// valid case
	v := newVirtiofsdFunc()
	err = v.valid()
	assert.NoError(err)

	v = newVirtiofsdFunc()
	v.path = ""
	err = v.valid()
	assert.Equal(errVirtiofsdDaemonPathEmpty, err)

	v = newVirtiofsdFunc()
	v.sourcePath = ""
	err = v.valid()
	assert.Equal(errVirtiofsdSourcePathEmpty, err)

	v = newVirtiofsdFunc()
	v.socketPath = ""
	err = v.valid()
	assert.Equal(errVirtiofsdSocketPathEmpty, err)

	v = newVirtiofsdFunc()
	v.sourcePath = "/foo/bar"
	err = v.valid()
	assert.Equal(errVirtiofsdSourceNotAvailable, err)
}
