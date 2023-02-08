// Copyright (c) 2020 Baidu Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"syscall"
	"testing"
	"time"

	"github.com/sirupsen/logrus"

	"github.com/containerd/fifo"
	"github.com/stretchr/testify/assert"
)

func TestNewTtyIOFifoReopen(t *testing.T) {
	var outr io.ReadWriteCloser
	var errr io.ReadWriteCloser
	var tty *ttyIO
	assert := assert.New(t)
	ctx := context.TODO()

	testDir := t.TempDir()

	fifoPath, err := os.MkdirTemp(testDir, "fifo-path-")
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
	tty, err = newTtyIO(ctx, "", "", "", stdout, stderr, false)
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

	checkFifoWrite(tty.io.Stdout())
	checkFifoRead(outr)
	checkFifoWrite(tty.io.Stderr())
	checkFifoRead(errr)

	err = outr.Close()
	assert.NoError(err)
	err = errr.Close()
	assert.NoError(err)

	// Make sure that writing to tty fifo will not get `EPIPE`
	// when the read side is closed
	checkFifoWrite(tty.io.Stdout())
	checkFifoWrite(tty.io.Stderr())

	// Reopen the fifo
	outr = createReadFifo(stdout)
	errr = createReadFifo(stderr)
	checkFifoRead(outr)
	checkFifoRead(errr)
}

func TestIoCopy(t *testing.T) {
	// This test fails on aarch64 regularly, temporarily skip it
	if runtime.GOARCH == "arm64" {
		t.Skip("Skip TestIoCopy for aarch64")
	}
	assert := assert.New(t)
	ctx := context.TODO()

	testBytes1 := []byte("Test1")
	testBytes2 := []byte("Test2")
	testBytes3 := []byte("Test3")

	testDir := t.TempDir()

	fifoPath, err := os.MkdirTemp(testDir, "fifo-path-")
	assert.NoError(err)
	dstStdoutPath := filepath.Join(fifoPath, "dststdout")
	dstStderrPath := filepath.Join(fifoPath, "dststderr")

	// test function: create pipes, and use ioCopy() to copy data from one set to the other
	// this function will be called multiple times, testing different combinations of closing order
	// in order to verify that closing a pipe doesn't break the copy for the others
	ioCopyTest := func(first, second, third string) {
		var srcStdinPath string
		if third != "" {
			srcStdinPath = filepath.Join(fifoPath, "srcstdin")
		}

		logErrorMsg := func(msg string) string {
			return "Error found while using order [" + first + " " + second + " " + third + "] - " + msg
		}

		exitioch := make(chan struct{})
		stdinCloser := make(chan struct{})

		createFifo := func(f string) (io.ReadCloser, io.WriteCloser) {
			reader, err := fifo.OpenFifo(ctx, f, syscall.O_RDONLY|syscall.O_CREAT|syscall.O_NONBLOCK, 0700)
			if err != nil {
				t.Fatal(err)
			}
			writer, err := fifo.OpenFifo(ctx, f, syscall.O_WRONLY|syscall.O_CREAT|syscall.O_NONBLOCK, 0700)
			if err != nil {
				reader.Close()
				t.Fatal(err)
			}
			return reader, writer
		}

		// create two sets of stdin, stdout and stderr pipes, to copy data from one to the other
		srcOutR, srcOutW := createFifo(filepath.Join(fifoPath, "srcstdout"))
		defer srcOutR.Close()
		defer srcOutW.Close()

		srcErrR, srcErrW := createFifo(filepath.Join(fifoPath, "srcstderr"))
		defer srcErrR.Close()
		defer srcErrW.Close()

		dstInR, dstInW := createFifo(filepath.Join(fifoPath, "dststdin"))
		defer dstInR.Close()
		defer dstInW.Close()

		dstOutR, err := fifo.OpenFifo(ctx, dstStdoutPath, syscall.O_RDONLY|syscall.O_CREAT|syscall.O_NONBLOCK, 0700)
		if err != nil {
			t.Fatal(err)
		}
		defer dstOutR.Close()
		dstErrR, err := fifo.OpenFifo(ctx, dstStderrPath, syscall.O_RDONLY|syscall.O_CREAT|syscall.O_NONBLOCK, 0700)
		if err != nil {
			t.Fatal(err)
		}
		defer dstErrR.Close()

		var srcInW io.WriteCloser
		if srcStdinPath != "" {
			srcInW, err = fifo.OpenFifo(ctx, srcStdinPath, syscall.O_WRONLY|syscall.O_CREAT|syscall.O_NONBLOCK, 0700)
			if err != nil {
				t.Fatal(err)
			}
			defer srcInW.Close()
		}

		tty, err := newTtyIO(ctx, "", "", srcStdinPath, dstStdoutPath, dstStderrPath, false)
		assert.NoError(err)
		defer tty.close()

		// start the ioCopy threads : copy from src to dst
		go ioCopy(logrus.WithContext(context.Background()), exitioch, stdinCloser, tty, dstInW, srcOutR, srcErrR)

		var firstW, secondW, thirdW io.WriteCloser
		var firstR, secondR, thirdR io.Reader
		getPipes := func(order string) (io.Reader, io.WriteCloser) {
			switch order {
			case "out":
				return dstOutR, srcOutW
			case "err":
				return dstErrR, srcErrW
			case "in":
				return dstInR, srcInW
			case "":
				return nil, nil
			}
			t.Fatal("internal error")
			return nil, nil
		}

		firstR, firstW = getPipes(first)
		secondR, secondW = getPipes(second)
		thirdR, thirdW = getPipes(third)

		checkFifoWrite := func(w io.Writer, b []byte, name string) {
			_, err := w.Write(b)
			if name == "in" && (name == third || name == second && first == "out") {
				// this is expected: when stdout is closed, ioCopy() will close stdin
				// so if "in" is after "out", we will get here
			} else {
				assert.NoError(err, logErrorMsg("Write error on std"+name))
			}
		}
		checkFifoRead := func(r io.Reader, b []byte, name string) {
			var err error
			buf := make([]byte, 5)
			done := make(chan struct{})
			timer := time.NewTimer(2 * time.Second)
			go func() {
				_, err = r.Read(buf)
				close(done)
			}()
			select {
			case <-done:
				assert.NoError(err, logErrorMsg("Error reading from std"+name))
				assert.Equal(b, buf, logErrorMsg("Value mismatch on std"+name))
			case <-timer.C:
				//t.Fatal(logErrorMsg("read fifo timeout on std" + name))
				if name == "in" && (name == third || name == second && first == "out") {
					// this is expected: when stdout is closed, ioCopy() will close stdin
					// so if "in" is after "out", we will get here
				} else {
					assert.Fail(logErrorMsg("read fifo timeout on std" + name))
				}
				return
			}
		}

		// check everything works without closed pipes
		checkFifoWrite(firstW, testBytes1, first)
		checkFifoRead(firstR, testBytes1, first)

		checkFifoWrite(secondW, testBytes2, second)
		checkFifoRead(secondR, testBytes2, second)

		if thirdW != nil {
			checkFifoWrite(thirdW, testBytes3, third)
			checkFifoRead(thirdR, testBytes3, third)
		}

		// write to each pipe, and close them immediately
		// the ioCopy function should copy the data, then stop the corresponding thread
		checkFifoWrite(firstW, testBytes1, first)
		firstW.Close()

		// need to make sure the Close() above is done before we continue
		time.Sleep(time.Second)

		checkFifoWrite(secondW, testBytes2, second)
		secondW.Close()

		if thirdW != nil {
			// need to make sure the Close() above is done before we continue
			time.Sleep(time.Second)

			checkFifoWrite(thirdW, testBytes3, third)
			thirdW.Close()
		}

		// wait for the end of the ioCopy
		timer := time.NewTimer(2 * time.Second)
		select {
		case <-exitioch:
			// now check that all data has been copied properly
			checkFifoRead(firstR, testBytes1, first)
			checkFifoRead(secondR, testBytes2, second)
			if thirdR != nil {
				checkFifoRead(thirdR, testBytes3, third)
			}
		case <-timer.C:
			t.Fatal(logErrorMsg("timeout waiting for ioCopy()"))
		}
	}

	// try the different combinations

	// tests without stdin
	ioCopyTest("out", "err", "")
	ioCopyTest("err", "out", "")

	// tests with stdin
	ioCopyTest("out", "err", "in")
	ioCopyTest("out", "in", "err")
	ioCopyTest("err", "out", "in")
	ioCopyTest("err", "in", "out")
	ioCopyTest("in", "out", "err")
	ioCopyTest("in", "err", "out")
}
