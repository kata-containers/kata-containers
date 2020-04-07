// Copyright (c) 2020 Baidu Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"io"
	"io/ioutil"
	"path/filepath"
	"syscall"
	"testing"
	"time"

	"github.com/containerd/fifo"
	"github.com/stretchr/testify/assert"
)

func TestNewTtyIOFifoReopen(t *testing.T) {
	var outr io.ReadWriteCloser
	var errr io.ReadWriteCloser
	var tty *ttyIO
	assert := assert.New(t)
	ctx := context.TODO()
	fifoPath, err := ioutil.TempDir(testDir, "fifo-path-")
	assert.NoError(err)
	stdout := filepath.Join(fifoPath, "stdout")
	stderr := filepath.Join(fifoPath, "stderr")

	createReadFifo := func(f string) io.ReadWriteCloser {
		rf, err := fifo.OpenFifo(ctx, f, syscall.O_RDONLY|syscall.O_CREAT|syscall.O_NONBLOCK, 0700)
		if err != nil {
			t.Fatal(err)
		}
		return rf
	}

	outr = createReadFifo(stdout)
	defer outr.Close()
	errr = createReadFifo(stderr)
	defer errr.Close()
	tty, err = newTtyIO(ctx, "", stdout, stderr, false)
	assert.NoError(err)
	defer tty.close()

	testBytes := []byte("T")
	checkFifoWrite := func(w io.Writer) {
		_, err = w.Write(testBytes)
		assert.NoError(err)
	}
	checkFifoRead := func(r io.Reader) {
		var err error
		buf := make([]byte, 1)
		done := make(chan struct{})
		timer := time.NewTimer(2 * time.Second)
		go func() {
			_, err = r.Read(buf)
			close(done)
		}()
		select {
		case <-done:
			assert.NoError(err)
			assert.Equal(buf, testBytes)
		case <-timer.C:
			t.Fatal("read fifo timeout")
		}
	}

	checkFifoWrite(tty.Stdout)
	checkFifoRead(outr)
	checkFifoWrite(tty.Stderr)
	checkFifoRead(errr)

	err = outr.Close()
	assert.NoError(err)
	err = errr.Close()
	assert.NoError(err)

	// Make sure that writing to tty fifo will not get `EPIPE`
	// when the read side is closed
	checkFifoWrite(tty.Stdout)
	checkFifoWrite(tty.Stderr)

	// Reopen the fifo
	outr = createReadFifo(stdout)
	errr = createReadFifo(stderr)
	checkFifoRead(outr)
	checkFifoRead(errr)
}
