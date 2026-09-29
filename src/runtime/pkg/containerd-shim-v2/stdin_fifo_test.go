// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0

package containerdshim

import (
	"bytes"
	"context"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"syscall"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/require"
)

type slowStdin struct {
	buffer  bytes.Buffer
	entered chan struct{}
	resume  chan struct{}
	once    sync.Once
}

func (s *slowStdin) Write(p []byte) (int, error) {
	s.once.Do(func() { close(s.entered) })
	<-s.resume
	return s.buffer.Write(p)
}

func (*slowStdin) Close() error { return nil }

func TestCloseIOConcurrent(t *testing.T) {
	for _, execID := range []string{"", "exec"} {
		t.Run("process-"+execID, func(t *testing.T) {
			s, request, _, done := fifoTestProcess(t, makeFIFO(t, "stdin"), &testStdin{}, execID)
			results := make([]<-chan error, 8)
			for i := range results {
				results[i] = closeTestIO(s, request)
			}
			for _, result := range results {
				require.NoError(t, awaitIOError(t, result))
			}
			awaitIO(t, done)
		})
	}
}

func TestCloseIODrainsLargeInput(t *testing.T) {
	path := makeFIFO(t, "stdin")
	guest := &slowStdin{entered: make(chan struct{}), resume: make(chan struct{})}
	var resume sync.Once
	release := func() { resume.Do(func() { close(guest.resume) }) }
	s, request, _, done := fifoTestProcess(t, path, guest, "exec")
	t.Cleanup(release)
	want := append(bytes.Repeat([]byte("buffered-input\n"), 150000), []byte("END\n")...)
	produced := make(chan error, 1)
	go func() {
		writer, err := os.OpenFile(path, syscall.O_WRONLY, 0)
		if err == nil {
			_, err = writer.Write(want)
			closeErr := writer.Close()
			if err == nil {
				err = closeErr
			}
		}
		produced <- err
	}()
	awaitIO(t, guest.entered)
	result := closeTestIO(s, request)
	deadline := time.After(5 * time.Second)
	for s.mu.TryLock() {
		s.mu.Unlock()
		select {
		case <-deadline:
			t.Fatal("CloseIO did not acquire the original service lock")
		default:
			runtime.Gosched()
		}
	}
	select {
	case err := <-result:
		t.Fatalf("CloseIO returned before input drained: %v", err)
	default:
	}
	release()
	require.NoError(t, awaitIOError(t, produced))
	require.NoError(t, awaitIOError(t, result))
	awaitIO(t, done)
	require.Equal(t, want, guest.buffer.Bytes())
}

func TestCloseIODrainsDataBufferedBeforeReader(t *testing.T) {
	path := makeFIFO(t, "stdin")
	holder, err := os.OpenFile(path, syscall.O_RDONLY|syscall.O_NONBLOCK, 0)
	require.NoError(t, err)
	defer holder.Close()
	writer, err := os.OpenFile(path, syscall.O_WRONLY|syscall.O_NONBLOCK, 0)
	require.NoError(t, err)
	want := []byte("buffered before the shim reader exists\n")
	_, err = writer.Write(want)
	require.NoError(t, err)
	require.NoError(t, writer.Close())
	guest := &testStdin{}
	s, request, _, done := fifoTestProcess(t, path, guest, "exec")
	require.NoError(t, awaitIOError(t, closeTestIO(s, request)))
	awaitIO(t, done)
	require.Equal(t, want, guest.Bytes())
}

func TestStdinFIFOEOFWithoutData(t *testing.T) {
	f, err := newStdinFIFO(context.Background(), makeFIFO(t, "stdin"))
	require.NoError(t, err)
	defer f.Close()
	require.NoError(t, f.finishOpen())
	n, err := f.Read(make([]byte, 1))
	require.Zero(t, n)
	require.ErrorIs(t, err, io.EOF)
}

func TestStdinFIFONormalEOFWithoutCloseIO(t *testing.T) {
	path := makeFIFO(t, "stdin")
	f, err := newStdinFIFO(context.Background(), path)
	require.NoError(t, err)
	defer f.Close()
	select {
	case <-f.ready:
		t.Fatal("reader opened before any producer connected")
	default:
	}
	writer, err := os.OpenFile(path, syscall.O_WRONLY, 0)
	require.NoError(t, err)
	defer writer.Close()
	want := []byte("normal producer input")
	_, err = writer.Write(want)
	require.NoError(t, err)
	require.NoError(t, writer.Close())
	got, err := io.ReadAll(f)
	require.NoError(t, err)
	require.Equal(t, want, got)
}

func TestCloseIOKeepsOutputAlive(t *testing.T) {
	s, request, host, stdinDone := fifoTestProcess(t, makeFIFO(t, "stdin"), &testStdin{}, "exec")
	host.close()
	awaitIO(t, stdinDone)
	outPath := makeFIFO(t, "stdout")
	host, err := newTtyIO(context.Background(), "", "", makeFIFO(t, "stdin"), outPath, "", false)
	require.NoError(t, err)
	output, err := os.OpenFile(outPath, syscall.O_RDONLY, 0)
	require.NoError(t, err)
	defer output.Close()
	guestOut, producer := io.Pipe()
	defer producer.Close()
	c := s.containers[request.ID]
	e, err := c.getExec(request.ExecID)
	require.NoError(t, err)
	e.ttyio, e.stdinCloser, e.exitIOch = host, make(chan struct{}), make(chan struct{})
	go ioCopy(logrus.NewEntry(logrus.New()), e.exitIOch, e.stdinCloser, host, e.stdinPipe, guestOut, nil)
	t.Cleanup(func() {
		producer.Close()
		host.close()
		awaitIO(t, e.exitIOch)
	})
	require.NoError(t, awaitIOError(t, closeTestIO(s, request)))
	select {
	case <-e.exitIOch:
		t.Fatal("CloseIO ended stdout before the guest closed it")
	default:
	}
	want := []byte("output after stdin EOF")
	written := make(chan error, 1)
	go func() {
		_, err := producer.Write(want)
		written <- err
	}()
	got := make([]byte, len(want))
	_, err = io.ReadFull(output, got)
	require.NoError(t, err)
	require.NoError(t, awaitIOError(t, written))
	require.Equal(t, want, got)
	require.NoError(t, producer.Close())
	awaitIO(t, e.exitIOch)
}

func TestStdinFIFOPreservesLiveWriter(t *testing.T) {
	path := makeFIFO(t, "stdin")
	f, err := newStdinFIFO(context.Background(), path)
	require.NoError(t, err)
	defer f.Close()
	writer, err := os.OpenFile(path, syscall.O_WRONLY, 0)
	require.NoError(t, err)
	defer writer.Close()
	require.NoError(t, f.finishOpen())
	want := []byte("producer writes after CloseIO finishes the open")
	_, err = writer.Write(want)
	require.NoError(t, err)
	require.NoError(t, writer.Close())
	got, err := io.ReadAll(f)
	require.NoError(t, err)
	require.Equal(t, want, got)
}

func TestStdinFIFOPinnedInode(t *testing.T) {
	path := makeFIFO(t, "stdin")
	f, err := newStdinFIFO(context.Background(), path)
	require.NoError(t, err)
	defer f.Close()
	require.NoError(t, os.Remove(path))
	require.NoError(t, syscall.Mkfifo(path, 0600))
	require.NoError(t, f.finishOpen())
	_, err = f.Read(make([]byte, 1))
	require.ErrorIs(t, err, io.EOF)
}

func TestStdinFIFOConcurrentFinishAndCancel(t *testing.T) {
	for i := 0; i < 32; i++ {
		f, err := newStdinFIFO(context.Background(), makeFIFO(t, "stdin"))
		require.NoError(t, err)
		var wg sync.WaitGroup
		for j := 0; j < 4; j++ {
			wg.Add(2)
			go func() {
				defer wg.Done()
				if err := f.finishOpen(); err != nil && err != context.Canceled {
					t.Error(err)
				}
			}()
			go func() {
				defer wg.Done()
				if err := f.Close(); err != nil {
					t.Error(err)
				}
			}()
		}
		wg.Wait()
		_, err = f.anchor.Stat()
		require.Error(t, err)
	}
}

func TestPipeIOMissingStdinReturnsError(t *testing.T) {
	_, err := newPipeIO(context.Background(), &stdio{
		Stdin: filepath.Join(t.TempDir(), "missing-stdin"),
	})
	require.Error(t, err)
}

func TestPipeIOPartialConstructionCleanup(t *testing.T) {
	for _, failedOutput := range []string{"stdout", "stderr"} {
		t.Run(failedOutput, func(t *testing.T) {
			stdio := &stdio{Stdin: makeFIFO(t, "stdin")}
			missing := filepath.Join(t.TempDir(), "missing-output")
			if failedOutput == "stdout" {
				stdio.Stdout = missing
			} else {
				stdio.Stdout, stdio.Stderr = makeFIFO(t, "stdout"), missing
			}
			for i := 0; i < 16; i++ {
				_, err := newPipeIO(context.Background(), stdio)
				require.Error(t, err)
			}
			deadline := time.After(5 * time.Second)
			for {
				fds, err := os.ReadDir("/proc/self/fd")
				require.NoError(t, err)
				var owned []string
				for _, fd := range fds {
					target, err := os.Readlink(filepath.Join("/proc/self/fd", fd.Name()))
					if err == nil && (target == stdio.Stdin || target == stdio.Stdout) {
						owned = append(owned, target)
					}
				}
				if len(owned) == 0 {
					break
				}
				select {
				case <-deadline:
					t.Fatalf("constructor rollback leaked FIFO descriptors: %v", owned)
				default:
					runtime.Gosched()
				}
			}
		})
	}
}
