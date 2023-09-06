// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"path"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestVirtiofsdStart(t *testing.T) {
	// nolint: govet
	type fields struct {
		path       string
		socketPath string
		cache      string
		extraArgs  []string
		sourcePath string
		PID        int
		ctx        context.Context
	}

	sourcePath := t.TempDir()
	socketDir := t.TempDir()

	socketPath := path.Join(socketDir, "socket.s")

	validConfig := fields{
		path:       "/usr/bin/virtiofsd-path",
		socketPath: socketPath,
		sourcePath: sourcePath,
	}
	NoDirectorySocket := validConfig
	NoDirectorySocket.socketPath = "/tmp/path/to/virtiofsd/socket.sock"

	// nolint: govet
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
				PID:        tt.fields.PID,
				ctx:        tt.fields.ctx,
			}
			ctx := context.Background()
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
		cache:      "never",
	}

	expected := "--syslog --cache=never --shared-dir=/run/kata-shared/foo --fd=123"
	args, err := v.args(123)
	assert.NoError(err)
	assert.Equal(expected, strings.Join(args, " "))

	expected = "--syslog --cache=never --shared-dir=/run/kata-shared/foo --fd=456"
	args, err = v.args(456)
	assert.NoError(err)
	assert.Equal(expected, strings.Join(args, " "))
}

func TestValid(t *testing.T) {
	a := assert.New(t)

	sourcePath := t.TempDir()
	socketDir := t.TempDir()

	socketPath := socketDir + "socket.s"

	newVirtiofsdFunc := func() *virtiofsd {
		return &virtiofsd{
			path:       "/usr/bin/virtiofsd",
			sourcePath: sourcePath,
			socketPath: socketPath,
			cache:      "auto",
		}
	}

	type fieldFunc func(v *virtiofsd)
	type assertFunc func(name string, assert *assert.Assertions, v *virtiofsd)

	// nolint: govet
	tests := []struct {
		name         string
		f            fieldFunc
		wantErr      error
		customAssert assertFunc
	}{
		{"valid case", nil, nil, nil},
		{"no path", func(v *virtiofsd) {
			v.path = ""
		}, errVirtiofsdDaemonPathEmpty, nil},
		{"no sourcePath", func(v *virtiofsd) {
			v.sourcePath = ""
		}, errVirtiofsdSourcePathEmpty, nil},
		{"no socketPath", func(v *virtiofsd) {
			v.socketPath = ""
		}, errVirtiofsdSocketPathEmpty, nil},
		{"source is not available", func(v *virtiofsd) {
			v.sourcePath = "/foo/bar"
		}, errVirtiofsdSourceNotAvailable, nil},
		{"invalid cache mode", func(v *virtiofsd) {
			v.cache = "foo"
		}, errVirtiofsdInvalidVirtiofsCacheMode("foo"), nil},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := newVirtiofsdFunc()
			if tt.f != nil {
				tt.f(v)
			}
			err := v.valid()
			if tt.wantErr != nil && err == nil {
				t.Errorf("test case %+s: virtiofsd.valid() should get error `%+v`, but got nil", tt.name, tt.wantErr)
			} else if tt.wantErr == nil && err != nil {
				t.Errorf("test case %+s: virtiofsd.valid() should get no erro, but got `%+v`", tt.name, err)
			} else if tt.wantErr != nil && err != nil {
				a.Equal(err.Error(), tt.wantErr.Error(), "test case %+s", tt.name)
			}

			if tt.customAssert != nil {
				tt.customAssert(tt.name, a, v)
			}
		})
	}
}
