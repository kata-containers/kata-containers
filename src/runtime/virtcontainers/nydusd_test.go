// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/base64"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNydusdStart(t *testing.T) {
	// nolint: govet
	type fields struct {
		pid             int
		path            string
		sockPath        string
		apiSockPath     string
		sourcePath      string
		debug           bool
		extraArgs       []string
		startFn         func(cmd *exec.Cmd) error
		waitFn          func() error
		setupShareDirFn func() error
	}

	sourcePath := t.TempDir()
	socketDir := t.TempDir()

	sockPath := filepath.Join(socketDir, "vhost-user.sock")
	apiSockPath := filepath.Join(socketDir, "api.sock")

	validConfig := fields{
		path:        "/usr/bin/nydusd",
		sockPath:    sockPath,
		apiSockPath: apiSockPath,
		sourcePath:  sourcePath,
		startFn: func(cmd *exec.Cmd) error {
			cmd.Process = &os.Process{}
			return nil
		},
		waitFn: func() error {
			return nil
		},
		setupShareDirFn: func() error { return nil },
	}
	SourcePathNoExist := validConfig
	SourcePathNoExist.sourcePath = "/tmp/path/to/nydusd/sourcepath"

	// nolint: govet
	tests := []struct {
		name    string
		fields  fields
		wantErr bool
	}{
		{"empty config", fields{}, true},
		{"directory source path not exist", SourcePathNoExist, true},
		{"valid config", validConfig, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			nd := &nydusd{
				path:            tt.fields.path,
				sockPath:        tt.fields.sockPath,
				apiSockPath:     tt.fields.apiSockPath,
				sourcePath:      tt.fields.sourcePath,
				extraArgs:       tt.fields.extraArgs,
				debug:           tt.fields.debug,
				pid:             tt.fields.pid,
				startFn:         tt.fields.startFn,
				waitFn:          tt.fields.waitFn,
				setupShareDirFn: tt.fields.setupShareDirFn,
			}
			ctx := context.Background()

			_, err := nd.Start(ctx, nil)
			if (err != nil) != tt.wantErr {
				t.Errorf("nydusd.Start() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
		})

	}

}
func TestNydusdArgs(t *testing.T) {
	assert := assert.New(t)
	nd := &nydusd{
		path:        "/usr/bin/nydusd",
		sockPath:    "/var/lib/vhost-user.sock",
		apiSockPath: "/var/lib/api.sock",
		debug:       true,
	}
	expected := "virtiofs --log-level debug --apisock /var/lib/api.sock --sock /var/lib/vhost-user.sock"
	args, err := nd.args()
	assert.NoError(err)
	assert.Equal(expected, strings.Join(args, " "))

	nd.debug = false
	expected = "virtiofs --log-level info --apisock /var/lib/api.sock --sock /var/lib/vhost-user.sock"
	args, err = nd.args()
	assert.NoError(err)
	assert.Equal(expected, strings.Join(args, " "))
}

func TestNydusdValid(t *testing.T) {
	assert := assert.New(t)

	sourcePath := t.TempDir()
	socketDir := t.TempDir()

	sockPath := filepath.Join(socketDir, "vhost-user.sock")
	apiSockPath := filepath.Join(socketDir, "api.sock")

	newNydsudFunc := func() *nydusd {
		return &nydusd{
			path:        "/usr/bin/nydusd",
			sourcePath:  sourcePath,
			sockPath:    sockPath,
			apiSockPath: apiSockPath,
		}
	}
	nd := newNydsudFunc()
	err := nd.valid()
	assert.NoError(err)

	nd = newNydsudFunc()
	nd.path = ""
	err = nd.valid()
	assert.Equal(errNydusdDaemonPathInvalid, err)

	nd = newNydsudFunc()
	nd.sockPath = ""
	err = nd.valid()
	assert.Equal(errNydusdSockPathInvalid, err)

	nd = newNydsudFunc()
	nd.apiSockPath = ""
	err = nd.valid()
	assert.Equal(errNydusdAPISockPathInvalid, err)

	nd = newNydsudFunc()
	nd.sourcePath = ""
	err = nd.valid()
	assert.Equal(errNydusdSourcePathInvalid, err)
}

func TestParseExtraOption(t *testing.T) {
	tests := []struct {
		name    string
		option  string
		wantErr bool
	}{
		{
			name:    "valid option",
			option:  "extraoption=" + base64.StdEncoding.EncodeToString([]byte("{\"source\":\"/path/to/bootstrap\",\"config\":\"config content\",\"snapshotdir\":\"/path/to/snapshotdir\"}")),
			wantErr: false,
		},
		{
			name:    "no extra option",
			option:  base64.StdEncoding.EncodeToString([]byte("{\"source\":/path/to/bootstrap,\"config\":config content,\"snapshotdir\":/path/to/snapshotdir}")),
			wantErr: true,
		},
		{
			name:    "no source",
			option:  "extraoption=" + base64.StdEncoding.EncodeToString([]byte("{\"config\":config content,\"snapshotdir\":/path/to/snapshotdir}")),
			wantErr: true,
		},
		{
			name:    "no config",
			option:  "extraoption=" + base64.StdEncoding.EncodeToString([]byte("{\"source\":/path/to/bootstrap,\"snapshotdir\":/path/to/snapshotdir}")),
			wantErr: true,
		},
		{
			name:    "no snapshotdir",
			option:  "extraoption=" + base64.StdEncoding.EncodeToString([]byte("{\"source\":/path/to/bootstrap,\"config\":config content}")),
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := parseExtraOption([]string{tt.option})
			if (err != nil) != tt.wantErr {
				t.Errorf("parseExtraOption error = %v, wantErr %v", err, tt.wantErr)
				return
			}
		})
	}
}

func TestCheckRafsMountPointValid(t *testing.T) {
	tests := []struct {
		mountPoint string
		valid      bool
	}{
		{
			mountPoint: "/rafs/xxxxaaa/lowerdir",
			valid:      true,
		},
		{
			mountPoint: "/",
			valid:      false,
		},
		{
			mountPoint: "/rafs",
			valid:      false,
		},
		{
			mountPoint: "/xxxx",
			valid:      false,
		},
		{
			mountPoint: "/rafs/aaaaa/xxxx",
			valid:      false,
		},
		{
			mountPoint: "/rafs//lowerdir",
			valid:      false,
		},
	}
	for _, tt := range tests {
		res := checkRafsMountPointValid(tt.mountPoint)
		if res != tt.valid {
			t.Errorf("test %v get %v, but want %v", tt, res, tt.valid)
		}
	}
}
