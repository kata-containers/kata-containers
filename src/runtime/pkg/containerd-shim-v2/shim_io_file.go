// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"io"
	"net/url"
	"os"
	"path/filepath"

	cioutil "github.com/containerd/containerd/pkg/ioutil"
)

var (
	_ IO = &fileIO{}
)

// fileIO only support write both stdout/stderr to the same file
type fileIO struct {
	outw io.WriteCloser
	errw io.WriteCloser
	path string
}

// openLogFile opens/creates a container log file with its directory.
func openLogFile(path string) (*os.File, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return nil, err
	}
	return os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0640)
}

func newFileIO(ctx context.Context, stdio *stdio, uri *url.URL) (*fileIO, error) {
	var outw, errw, f io.WriteCloser
	var err error

	logFile := uri.Path
	if f, err = openLogFile(logFile); err != nil {
		return nil, err
	}

	if stdio.Stdout != "" {
		outw = cioutil.NewSerialWriteCloser(f)
	}

	if !stdio.Console && stdio.Stderr != "" {
		errw = cioutil.NewSerialWriteCloser(f)
	}

	return &fileIO{
		path: logFile,
		outw: outw,
		errw: errw,
	}, nil
}

func (fi *fileIO) Close() error {
	if fi.outw != nil {
		return wc(fi.outw)
	} else if fi.errw != nil {
		return wc(fi.errw)
	}
	return nil
}

func (fi *fileIO) Stdin() io.ReadCloser {
	return nil
}

func (fi *fileIO) Stdout() io.Writer {
	return fi.outw
}

func (fi *fileIO) Stderr() io.Writer {
	return fi.errw
}
